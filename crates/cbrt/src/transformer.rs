use crate::parser::{CBRT_SYNC, ParseOutcome, SessionState, parse_at};
use async_trait::async_trait;
use cycbox_sdk::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub const CBRT_TRANSFORMER_ID: &str = "cbrt_transformer";

/// Sentinel metadata key the codec writes on every successful decode. If present
/// the transformer skips parsing to avoid double-populating values/metadata when
/// both the codec and the transformer are configured on the same connection.
const CODEC_SENTINEL: &str = "datatype";

#[derive(Debug, Default)]
pub struct CbrtTransformer {
    /// Per-connection session state. Each message-based connection (UDP peer, MQTT
    /// client) gets its own slot so wrap counters, sequence drops, and the ts
    /// anchor track that source independently.
    sessions: Mutex<HashMap<u32, SessionState>>,
}

impl CbrtTransformer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Transformer for CbrtTransformer {
    fn on_receive(&self, message: &mut Message) -> Result<(), CycBoxError> {
        // Skip if the codec already parsed this frame on the same pipeline.
        if message.metadata_value(CODEC_SENTINEL).is_some() {
            return Ok(());
        }

        if message.payload.len() < CBRT_SYNC.len()
            || message.payload[..CBRT_SYNC.len()] != CBRT_SYNC
        {
            return Err(CycBoxError::InvalidFormat(
                "cbrt: payload does not start with CBRT sync word".to_string(),
            ));
        }

        let arrival_us = if message.timestamp != 0 {
            message.timestamp
        } else {
            Message::current_timestamp()
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| CycBoxError::InvalidFormat("cbrt: session lock poisoned".to_string()))?;
        let state = sessions.entry(message.connection_id).or_default();

        match parse_at(state, &message.payload, 0, arrival_us) {
            ParseOutcome::Complete {
                frame_end,
                message: parsed,
            } => {
                if frame_end != message.payload.len() {
                    log::debug!(
                        "cbrt transformer: {} trailing byte(s) after frame on conn {}",
                        message.payload.len() - frame_end,
                        message.connection_id
                    );
                }
                message.values.extend(parsed.values);
                message.metadata.extend(parsed.metadata);
                if message.contents.is_empty() {
                    message.contents = parsed.contents;
                } else {
                    message.contents.extend(parsed.contents);
                }
                Ok(())
            }
            ParseOutcome::NeedMore => Err(CycBoxError::InvalidFormat(
                "cbrt: payload truncated mid-frame".to_string(),
            )),
            ParseOutcome::Reject => {
                // Reset on reject so a corrupted message doesn't poison the
                // session — the next valid frame establishes a fresh session.
                state.reset();
                Err(CycBoxError::InvalidFormat(
                    "cbrt: frame validation failed".to_string(),
                ))
            }
        }
    }
}

#[async_trait]
impl Manifestable for CbrtTransformer {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: CBRT_TRANSFORMER_ID.to_string(),
            name: l10n.get(locale, "cbrt-transformer"),
            description: l10n.get(locale, "cbrt-transformer-description"),
            category: PluginCategory::Transformer,
            config_schema: vec![FormGroup {
                key: CBRT_TRANSFORMER_ID.to_string(),
                label: l10n.get(locale, "cbrt-transformer"),
                description: Some(cbrt_transformer_config_description(locale)),
                fields: vec![],
                condition: None,
            }],
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for CbrtTransformer {}

fn cbrt_transformer_config_description(locale: &str) -> String {
    if locale.starts_with("zh") {
        r#"
**CBRT 转换器 — 面向消息型传输**

把单个完整的 CBRT 帧从 `message.payload` 中解析出来，输出与 [`cbrt_codec`] 完全一致的 `values` / `metadata` / `contents`。专为 UDP、MQTT 等**消息型传输**设计：这类传输按报文交付数据、不经过 `Codec` 流式解码，因此需要在 Transformer 层完成单帧解析。

#### 行为

- **payload 必须以同步字 `"CBRT"` 起始**，且恰好为一个完整帧。允许末尾少量多余字节（仅记日志），但不会扫描多帧。
- 维护**按 connection_id 隔离的会话状态**，覆盖时间戳回卷、序号丢包检测、抖动锚点等与流式 codec 相同的语义。
- **去重保护**：若 `metadata` 中已存在 `cbrt_datatype`（由 codec 写入的指纹），转换器**直接跳过**——这样即便用户同时启用了 `cbrt_codec` 与 `cbrt_transformer` 也不会重复解析或重复写入 values。
- 解析失败（同步字不匹配、字段越界、CRC 错误等）会重置该连接的会话状态并返回 `InvalidFormat`。
- `on_send` 为空——发送侧保持原样，由 codec 或上层负责构帧。
"#
        .to_string()
    } else {
        r#"
**CBRT Transformer — for message-based transports**

Parses a single complete CBRT frame out of `message.payload` and produces the
same `values` / `metadata` / `contents` as [`cbrt_codec`]. Intended for
**message-based transports** (UDP, MQTT, …) where each transport message
delivers one datagram and the `Codec` layer is bypassed — so frame parsing
needs to happen in the Transformer instead.

#### Behaviour

- The payload MUST start with the `"CBRT"` sync word and SHOULD contain exactly
  one frame. Trailing bytes are tolerated (logged at debug); multi-frame
  payloads are not scanned.
- Maintains **per-connection_id session state** — timestamp wrap detection,
  sequence-drop detection, and the jitter-anchored timestamp, matching the
  stream codec's semantics.
- **Double-parse guard**: if `metadata` already contains `cbrt_datatype`
  (the codec's fingerprint), the transformer is a no-op. Configuring both
  `cbrt_codec` and `cbrt_transformer` on the same pipeline is safe.
- A failed parse (bad sync, truncation, CRC mismatch …) resets that
  connection's session state and returns `InvalidFormat`.
- `on_send` is a no-op — TX is raw passthrough, matching the codec.
"#
        .to_string()
    }
}
