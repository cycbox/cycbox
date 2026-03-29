use crate::codec::{
    COBS_CODEC_ID, CobsCodec, LINE_CODEC_ID, LineCodec, PASSTHROUGH_CODEC_ID, PassthroughCodec,
    SLIP_CODEC_ID, SlipCodec, TIMEOUT_CODEC_ID, TimeoutCodec,
};
use crate::lua::RuntimeLuaFunctionRegistrar;
use crate::transformer::{
    CSV_TRANSFORMER_ID, CsvTransformer, DISABLE_TRANSFORMER_ID, DisableTransformer,
    JSON_TRANSFORMER_ID, JsonTransformer,
};
use async_trait::async_trait;
use cycbox_engine::DEFAULT_LUA_SCRIPT;
use cycbox_sdk::message::lua_functions::MessageLuaHelper;
use cycbox_sdk::prelude::*;
use serialport_transport::{SERIAL_PORT_TRANSPORT_ID, SerialTransport};
use std::sync::Arc;
use std::time::Duration;

const RUNTIME_RUN_MODE_ID: &str = "runtime";

#[derive(Clone)]
pub struct RuntimeRunMode {
    message_input_registry: Arc<MessageInputRegistry>,
    lua_registry: Arc<LuaFunctionRegistry>,
}

impl RuntimeRunMode {
    pub fn new() -> Self {
        let registry = MessageInputRegistry::new();

        let mut lua_registry = LuaFunctionRegistry::new();
        lua_registry.register(Box::new(MessageLuaHelper));
        lua_registry.register(Box::new(RuntimeLuaFunctionRegistrar));

        Self {
            message_input_registry: Arc::new(registry),
            lua_registry: Arc::new(lua_registry),
        }
    }
}

impl Default for RuntimeRunMode {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Manifestable for RuntimeRunMode {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = cycbox_engine::l10n::get_l10n();
        let mut manifest = Manifest {
            id: RUNTIME_RUN_MODE_ID.to_string(),
            name: l10n.get(locale, "app-name"),
            description: l10n.get(locale, "app-description"),
            category: PluginCategory::RunMode,
            config_schema: vec![FormGroup {
                key: "app".to_string(),
                label: l10n.get(locale, "app-basic-config"),
                description: Some(cycbox_run_mode_config_description(locale)),
                condition: None,
                fields: vec![],
            }],
            lua_script: Some(DEFAULT_LUA_SCRIPT.to_string()),
            ..Default::default()
        };

        // Build transport list conditionally based on enabled features
        let transports: Vec<Box<dyn Transport>> = vec![Box::new(SerialTransport)];
        let mut transports_manifest = vec![];
        for transport in &transports {
            transports_manifest.push(transport.manifest(locale).await);
        }
        manifest.with_groups(
            "transport",
            &l10n.get(locale, "app-transport-config"),
            transports_manifest,
        );
        let codecs: Vec<Box<dyn Codec>> = vec![
            Box::new(TimeoutCodec::default()),
            Box::new(LineCodec::default()),
            Box::new(CobsCodec::default()),
            Box::new(SlipCodec::default()),
            Box::new(PassthroughCodec),
        ];
        let mut codecs_manifest = vec![];
        for codec in &codecs {
            codecs_manifest.push(codec.manifest(locale).await);
        }
        manifest.with_groups(
            "codec",
            &l10n.get(locale, "app-codec-config"),
            codecs_manifest,
        );

        let transformers: Vec<Box<dyn Transformer>> = vec![
            Box::new(DisableTransformer::new()),
            Box::new(CsvTransformer::new()),
            Box::new(JsonTransformer::new()),
        ];
        let mut transformers_manifest = vec![];
        for transformer in &transformers {
            transformers_manifest.push(transformer.manifest(locale).await);
        }
        manifest.with_groups(
            "transformer",
            &l10n.get(locale, "app-transformer-config"),
            transformers_manifest,
        );
        manifest.with_encoding_field(&l10n.get(locale, "app-encoding-config"));
        manifest
    }
}

#[async_trait]
impl RunMode for RuntimeRunMode {
    async fn create_transport(
        &self,
        id: &str,
        configs: &[FormGroup],
        codec: Box<dyn Codec>,
        timeout: Duration,
    ) -> Result<Box<dyn MessageTransport>, CycBoxError> {
        let transport = match id {
            SERIAL_PORT_TRANSPORT_ID => SerialTransport.connect(configs, codec, timeout).await?,
            _ => {
                return Err(CycBoxError::Unsupported(format!("transport type: {}", id)));
            }
        };
        Ok(transport)
    }

    async fn create_transformer(
        &self,
        id: &str,
        configs: &[FormGroup],
    ) -> Result<Option<Box<dyn Transformer>>, CycBoxError> {
        let mut transformer: Box<dyn Transformer> = match id {
            DISABLE_TRANSFORMER_ID => Box::new(DisableTransformer::new()),
            CSV_TRANSFORMER_ID => Box::new(CsvTransformer::new()),
            JSON_TRANSFORMER_ID => Box::new(JsonTransformer::new()),
            _ => Box::new(DisableTransformer::new()),
        };
        transformer.config(configs).await?;
        Ok(Some(transformer))
    }

    async fn create_codec(
        &self,
        id: &str,
        configs: &[FormGroup],
    ) -> Result<Box<dyn Codec>, CycBoxError> {
        let mut codec: Box<dyn Codec> = match id {
            TIMEOUT_CODEC_ID => Box::new(TimeoutCodec::default()),
            LINE_CODEC_ID => Box::new(LineCodec::default()),
            COBS_CODEC_ID => Box::new(CobsCodec::default()),
            SLIP_CODEC_ID => Box::new(SlipCodec::default()),
            PASSTHROUGH_CODEC_ID => Box::new(PassthroughCodec),
            _ => return Err(CycBoxError::Unsupported(format!("codec type: {}", id))),
        };
        codec.config(configs).await?;
        Ok(codec)
    }

    fn message_input_registry(&self) -> &MessageInputRegistry {
        &self.message_input_registry
    }

    fn lua_helper_registry(&self) -> &LuaFunctionRegistry {
        &self.lua_registry
    }
}

fn cycbox_run_mode_config_description(locale: &str) -> String {
    if locale.starts_with("zh") {
        r#"
**1. 传输层 (Transport)：设置与设备通讯的方式**
- **串口 (Serial)**: 通过 COM/TTY 端口直接连接。
- **网络**: TCP 客户端/服务端, UDP, WebSocket 客户端/服务端，其中服务端只维护单个客户端连接。
- **MQTT**: 支持 MQTT 以及 MQTT over WebSocket，支持 TLS 加密连接。

**2. 编解码器 (Codec)：接收时从数据流解码出消息帧 (Frame)；发送时将消息帧编码成发送数据流**
- **透传 (Passthrough)**: 原始数据，不进行分包。
- **行模式 (Line)**: 按换行符 (`\n`, `\r\n`) 分包。
- **超时 (Timeout)**: 超过指定时间无数据则分包。
- **帧模式 (Frame)**: 。
- **Modbus**: 专用于 Modbus RTU/TCP 协议。

**3. 转换器 (Transformer)：接收时从消息内容（Payload）解释出数据；发送时如果消息帧存在数据则编码到消息内容**
- **CSV**: 以空格/逗号分隔的值，自动检测类型
- **JSON**: JSON 对象键值对。
- **禁用**: 使用自定义 Lua 脚本进行复杂转换。
"#
        .to_string()
    } else {
        r#"
**1. Transport: Set up communication methods with devices**
- **Serial**: Direct connection via COM/TTY ports.
- **Network**: TCP Client/Server, UDP, WebSocket Client/Server (Server maintains single client connection only).
- **MQTT**: Supports MQTT and MQTT over WebSocket, supports TLS encrypted connections.

**2. Codec: Decodes message frames from stream on receive; encodes frames to stream on send**
- **Passthrough**: Raw data, no packet splitting.
- **Line**: Splits by newline characters (`\n`, `\r\n`).
- **Timeout**: Splits if no data received for specified time.
- **Frame**: Splits by specific start and end markers.
- **Modbus**: Specialized for Modbus RTU/TCP protocols.

**3. Transformer: Interprets data from message payload on receive; encodes data to payload if frame contains data on send**
- **CSV**: Space/comma separated values with auto type detection.
- **JSON**: JSON object key-value pairs.
- **Disable**: Use custom Lua scripts for complex transformations.
"#
        .to_string()
    }
}
