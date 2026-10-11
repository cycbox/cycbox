//! The engine half of the transport sink: an `EVENT` a transport pushes from inside
//! `connect` reaches the Lua `on_event` hook stamped with its connection id, a `LOG`
//! becomes a log line, and a `REQUEST` sent on the TX path goes to the transport's
//! command handler instead of the codec (skipping Lua `on_send`). What a transport says
//! while a session fails, or while a connect fails, is shown straight away rather than
//! once some later session is up.

use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_engine::Engine;
use cycbox_sdk::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);
const FAKE_ID: &str = "fake";

/// Everything the fake transport saw, shared with the test.
#[derive(Default)]
struct Seen {
    sent: Vec<Message>,
    commands: Vec<String>,
}

/// How the fake transport behaves.
#[derive(Clone, Copy)]
enum Scenario {
    /// Connects, then idles.
    Healthy,
    /// Connects, then fails its first `recv` asking for the slow (minutes-long) backoff,
    /// saying why on the sink first.
    DropsSession,
    /// Every connect fails, saying why on the sink first.
    RejectsConnect,
}

struct FakeRunMode {
    scenario: Scenario,
    seen: Arc<Mutex<Seen>>,
    message_inputs: MessageInputRegistry,
    lua_helpers: LuaFunctionRegistry,
}

#[async_trait]
impl Manifestable for FakeRunMode {
    async fn manifest(&self, _locale: &str) -> Manifest {
        Manifest::default()
    }
}

#[async_trait]
impl RunMode for FakeRunMode {
    async fn create_transport(
        &self,
        _id: &str,
        _configs: &[FormGroup],
        _codec: Box<dyn Codec>,
        ctx: TransportContext,
    ) -> Result<Box<dyn MessageTransport>, CycBoxError> {
        if let Scenario::RejectsConnect = self.scenario {
            ctx.sink.warn("fake transport: password rejected");
            return Err(CycBoxError::Connection("rejected".into()));
        }
        // Reported before `connect` returns: the sink must already be live.
        ctx.sink.info("fake transport: listening");
        ctx.sink.event(
            "client_connected",
            vec![Value::new_string("peer", "10.0.0.2:5000")],
        );
        let drops = matches!(self.scenario, Scenario::DropsSession);
        Ok(Box::new(FakeTransport {
            seen: self.seen.clone(),
            dropping: drops.then_some(ctx.sink),
        }))
    }

    async fn create_transformer(
        &self,
        _id: &str,
        _configs: &[FormGroup],
    ) -> Result<Option<Box<dyn Transformer>>, CycBoxError> {
        Ok(None)
    }

    async fn create_codec(
        &self,
        _id: &str,
        _configs: &[FormGroup],
    ) -> Result<Box<dyn Codec>, CycBoxError> {
        Ok(Box::new(NullCodec))
    }

    fn message_input_registry(&self) -> &MessageInputRegistry {
        &self.message_inputs
    }

    fn lua_helper_registry(&self) -> &LuaFunctionRegistry {
        &self.lua_helpers
    }
}

struct FakeTransport {
    seen: Arc<Mutex<Seen>>,
    /// Set in [`Scenario::DropsSession`]: the sink to explain the failure on.
    dropping: Option<TransportSink>,
}

#[async_trait]
impl MessageTransport for FakeTransport {
    async fn recv(&mut self) -> Result<Option<Message>, CycBoxError> {
        match self.dropping.take() {
            Some(sink) => {
                sink.warn("fake transport: kicked by another client");
                Err(CycBoxError::ConnectionBackoff("kicked".into()))
            }
            None => std::future::pending().await,
        }
    }

    async fn send(&mut self, message: &mut Message) -> Result<(), CycBoxError> {
        self.seen.lock().unwrap().sent.push(message.clone());
        Ok(())
    }

    async fn handle_command(&mut self, command: &Message) -> Option<Message> {
        let name = command.get_command();
        self.seen.lock().unwrap().commands.push(name.clone());
        match name.as_str() {
            "probe_ok" => Some(MessageBuilder::response_success(command).build()),
            "probe_fail" => Some(MessageBuilder::response_error(command, "bus is busy").build()),
            _ => None,
        }
    }
}

struct NullCodec;

#[async_trait]
impl Manifestable for NullCodec {
    async fn manifest(&self, _locale: &str) -> Manifest {
        Manifest::default()
    }
}

#[async_trait]
impl Configurable for NullCodec {
    async fn config(&mut self, _configs: &[FormGroup]) -> Result<(), CycBoxError> {
        Ok(())
    }
}

impl Codec for NullCodec {
    fn decode(&mut self, _src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        Ok(None)
    }

    fn encode(&mut self, _item: &mut Message) -> Result<(), CycBoxError> {
        Ok(())
    }
}

fn text_field(key: &str, value: &str) -> FormField {
    FormField {
        key: key.to_string(),
        field_type: FieldType::TextInput,
        label: key.to_string(),
        description: None,
        values: Some(vec![FormValue::Text(value.to_string())]),
        options: None,
        is_required: true,
        condition: None,
        span: 12,
    }
}

fn manifest(lua: &str) -> Manifest {
    let app = FormGroup {
        key: "app".to_string(),
        label: "app".to_string(),
        description: None,
        condition: None,
        fields: vec![
            text_field("app_transport", FAKE_ID),
            text_field("app_codec", FAKE_ID),
        ],
    };
    Manifest {
        configs: vec![vec![app]],
        lua_script: Some(lua.to_string()),
        ..Default::default()
    }
}

async fn start(lua: &str) -> (Engine, broadcast::Receiver<Message>, Arc<Mutex<Seen>>) {
    start_with(Scenario::Healthy, lua).await
}

async fn start_with(
    scenario: Scenario,
    lua: &str,
) -> (Engine, broadcast::Receiver<Message>, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let run_mode = Arc::new(FakeRunMode {
        scenario,
        seen: seen.clone(),
        message_inputs: MessageInputRegistry::new(),
        lua_helpers: LuaFunctionRegistry::new(),
    });
    let engine = Engine::new(run_mode, false);
    let rx = engine.subscribe();
    engine
        .start("test", Some(manifest(lua)))
        .await
        .expect("engine starts");
    (engine, rx, seen)
}

/// Text of a broadcast log line, if `message` is one.
fn log_text(message: &Message) -> Option<String> {
    if message.message_type != MESSAGE_TYPE_LOG {
        return None;
    }
    let content = message.contents.first()?;
    Some(String::from_utf8_lossy(&content.payload).into_owned())
}

/// Waits for a broadcast log line containing `needle` and returns that message.
async fn wait_for_log(rx: &mut broadcast::Receiver<Message>, needle: &str) -> Message {
    timeout(WAIT, async {
        loop {
            let message = rx.recv().await.expect("broadcast open");
            if log_text(&message).is_some_and(|t| t.contains(needle)) {
                return message;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no log line containing {needle:?}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn sink_event_reaches_lua_on_event_and_log_is_attributed() {
    let lua = r#"
function on_event()
  log("info", "on_event " .. message.frame .. " conn=" .. message.connection_id
      .. " peer=" .. message:get_value("peer"))
  return false
end
"#;
    let (engine, mut rx, _seen) = start(lua).await;

    let line = wait_for_log(&mut rx, "fake transport: listening").await;
    assert_eq!(
        line.connection_id, 0,
        "a sink log line belongs to its connection"
    );

    wait_for_log(
        &mut rx,
        "on_event client_connected conn=0 peer=10.0.0.2:5000",
    )
    .await;

    engine.stop("test").await.expect("engine stops");
}

#[tokio::test(flavor = "multi_thread")]
async fn request_on_tx_path_goes_to_handle_command() {
    let lua = r#"
function on_send()
  log("info", "on_send saw " .. message.message_type)
  return false
end
"#;
    let (engine, mut rx, seen) = start(lua).await;
    wait_for_log(&mut rx, "fake transport: listening").await;

    let request = |command: &str| {
        MessageBuilder::request(1, command, 0, 0)
            .connection_id(0)
            .build()
    };
    engine
        .send_messages(vec![
            request("probe_ok"),
            request("probe_fail"),
            request("probe_unknown"),
        ])
        .await;

    wait_for_log(&mut rx, "command 'probe_ok' succeeded").await;
    wait_for_log(&mut rx, "command 'probe_fail' failed: bus is busy").await;
    wait_for_log(&mut rx, "has no handler for command 'probe_unknown'").await;

    // Ordinary TX still goes through on_send and the transport's send().
    let tx = MessageBuilder::tx(0, PayloadType::Binary, b"hi".to_vec(), b"hi".to_vec()).build();
    engine.send_messages(vec![tx]).await;
    let first_on_send = wait_for_log(&mut rx, "on_send saw").await;
    let text = log_text(&first_on_send).unwrap();
    assert!(
        text.ends_with("on_send saw tx"),
        "on_send must not see REQUEST messages, got {text:?}"
    );
    // on_send runs in the engine task, before the connection task has the message; the
    // TX confirmation is what says the transport took it.
    timeout(WAIT, async {
        while rx.recv().await.expect("broadcast open").message_type != MESSAGE_TYPE_TX {}
    })
    .await
    .expect("no TX confirmation");

    engine.stop("test").await.expect("engine stops");

    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.commands,
        ["probe_ok", "probe_fail", "probe_unknown"],
        "every REQUEST reaches handle_command, in order"
    );
    assert_eq!(seen.sent.len(), 1, "a REQUEST is never sent as data");
    assert_eq!(seen.sent[0].payload, b"hi");
}

/// A session that ends asking for the slow backoff waits minutes before the next
/// connect. Its last words must not wait with it.
#[tokio::test(flavor = "multi_thread")]
async fn sink_is_read_while_a_dropped_session_backs_off() {
    let (engine, mut rx, _seen) = start_with(Scenario::DropsSession, "").await;

    let line = wait_for_log(&mut rx, "fake transport: kicked by another client").await;
    assert_eq!(line.connection_id, 0);
    wait_for_log(&mut rx, "will not retry for").await;

    engine.stop("test").await.expect("engine stops");
}

/// A connect that fails never starts a session, so nothing inside one would ever read
/// what the transport said; it has to come out, ahead of the engine's own retry line.
#[tokio::test(flavor = "multi_thread")]
async fn sink_is_read_after_a_failed_connect() {
    let (engine, mut rx, _seen) = start_with(Scenario::RejectsConnect, "").await;

    wait_for_log(&mut rx, "fake transport: password rejected").await;
    wait_for_log(&mut rx, "transport error").await;

    engine.stop("test").await.expect("engine stops");
}
