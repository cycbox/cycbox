cbrt-codec = CBRT 实时编解码
cbrt-codec-description = 仅解码：解析 CBRT（CycBox Realtime）流式传感数据，支持多通道及可选时间戳。发送侧为原始透传，不附加任何帧头。
cbrt-transformer = CBRT 实时转换器
cbrt-transformer-description = 在 UDP / MQTT 等消息型传输上从 message payload 中解析单个 CBRT 帧，产出与 codec 一致的 values / metadata / contents。若 codec 已在同一管线上解过帧则自动跳过，可与 codec 同时启用。
