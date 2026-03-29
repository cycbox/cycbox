use cycbox_sdk::prelude::*;

#[derive(Clone)]
pub struct EngineState {
    pub manifest: Manifest,
    pub running: bool,
    pub connection_count: usize,
}

impl From<EngineState> for MessageBuilder {
    fn from(state: EngineState) -> Self {
        let manifest_json =
            serde_json::to_string(&state.manifest).unwrap_or_else(|_| "{}".to_string());
        MessageBuilder::event("state_change")
            .add_value(Value::new_string("manifest", manifest_json))
            .add_value(Value::new_boolean("running", state.running))
            .add_value(Value::new_u64(
                "connection_count",
                state.connection_count as u64,
            ))
    }
}
