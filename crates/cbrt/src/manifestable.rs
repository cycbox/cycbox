use crate::codec::{CBRT_CODEC_ID, CbrtCodec};
use async_trait::async_trait;
use cycbox_sdk::prelude::*;

#[async_trait]
impl Manifestable for CbrtCodec {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: CBRT_CODEC_ID.to_string(),
            name: l10n.get(locale, "cbrt-codec"),
            description: l10n.get(locale, "cbrt-codec-description"),
            category: PluginCategory::Codec,
            config_schema: vec![FormGroup {
                key: CBRT_CODEC_ID.to_string(),
                label: l10n.get(locale, "cbrt-codec"),
                description: Some(cbrt_codec_config_description(locale)),
                fields: vec![],
                condition: None,
            }],
            ..Default::default()
        }
    }
}

fn cbrt_codec_config_description(locale: &str) -> String {
    if locale.starts_with("zh") {
        r#"
**CBRT — CycBox 实时协议**

仅解码：解析具有 `"CBRT"` 同步字（`43 42 52 54`）的紧凑、长度定界、多通道实时采样帧。

#### 帧结构

```
"CBRT" | Flags+Datatype | Channels | [Seq] | [Ts µs] | [Period µs] | PayloadLen | Payload | [CRC-16]
```

- **Flags** (高 4 位): `has_ts | has_period | has_crc | has_seq`
- **Datatype** (低 4 位): `u8 / i8 / u16 / i16 / u32 / i32 / u64 / i64 / f32 / f64 / q15 / q31 / bf16 / f16 / bool-packed`（0xF 保留）
- **Channels**: 1–64，每帧所有通道共享同一数据类型
- **Payload**: 通道交错的样本数组（小端）
- **CRC**: CRC-16/MODBUS（多项式 `0x8005`，初值 `0xFFFF`，输入/输出反射），覆盖从 Flags 到 Payload 末尾。与 modbus-rtu 使用同一算法

#### 行为

- **解码**：把每个有效帧解析成 `N × M` 个 `Value`（N 个通道 × M 个样本），id 为 `ch0`..`chN-1`。如果设置了 `has_period`，每个样本的时间戳为 `base + i × period_us`；否则全部使用同一个 `base`。
- **抖动补偿**：当帧带有 `has_ts` 时，会按 `offset = min(arrival - ts)` 在整个会话内持续跟踪主机/设备时钟偏移——传输延迟较低的帧会把锚点逐步向真实的单程延迟下界拉紧，因此首帧因 TCP/BLE 缓冲带来的延迟不会污染后续样本。每帧的 `base = ts + offset`，使帧间间隔由 MCU 时钟决定。未带 `has_ts` 的帧仍然使用 `arrival` 时刻。注意：锚点收紧时新帧的时间戳可能比上一帧的末样本略早。
- **会话**：首个有效帧确定标志位/数据类型/通道数；任何变化都视为新会话（重置 wrap 计数、last seq）。
- **CRC 失败**：丢弃当前候选，从下一个 sync 字开始重同步（中间字节可能正好命中 sync）。
- **payload_length=0**：作为 keep-alive 输出一条空 `values` 的消息，仍然带 ts/period 等元数据。
- **编码**：原始透传，不附加任何字节。可以通过 `send_raw` 命令向 MCU 发送启动指令。
"#
        .to_string()
    } else {
        r#"
**CBRT — CycBox Realtime Protocol**

Decode-only codec for the compact, length-delimited, multi-channel realtime sample protocol identified by the sync word `"CBRT"` (`43 42 52 54`).

#### Frame structure

```
"CBRT" | Flags+Datatype | Channels | [Seq] | [Ts µs] | [Period µs] | PayloadLen | Payload | [CRC-16]
```

- **Flags** (high 4 bits): `has_ts | has_period | has_crc | has_seq`
- **Datatype** (low 4 bits): `u8 / i8 / u16 / i16 / u32 / i32 / u64 / i64 / f32 / f64 / q15 / q31 / bf16 / f16 / bool-packed` (0xF reserved)
- **Channels**: 1–64. All channels in a frame share one datatype.
- **Payload**: channel-interleaved samples, little-endian.
- **CRC**: CRC-16/MODBUS (poly `0x8005`, init `0xFFFF`, input/output reflected) over the flags byte through end of payload — same algorithm as modbus-rtu.

#### Behaviour

- **Decode** materializes each frame as `N × M` `Value`s (N channels × M samples) with ids `ch0`..`chN-1`. With `has_period`, sample i carries timestamp `base + i × period_us`; without, every sample shares the same `base`.
- **Jitter compensation:** when frames carry `has_ts`, the host/device clock offset is tracked as `offset = min(arrival - ts)` across the whole session — a frame that arrives with low transport delay ratchets the anchor toward the true one-way latency floor, so a slow first frame (TCP handshake, BLE buffering) doesn't poison every subsequent sample. Each frame's `base = ts + offset`, so inter-frame spacing is driven by the MCU clock rather than transport/scheduling jitter. Frames without `has_ts` fall back to `arrival`. Note: when the anchor tightens, a new frame's timestamps may land slightly earlier than the previous frame's last sample.
- **Session** is established by the first valid frame; any change in flag profile / datatype / channel count starts a new session (wrap counter and last-seq are reset).
- **CRC failure** skips to the next sync word — a `CBRT` sequence that occurs mid-frame becomes the new candidate.
- **payload_length=0** is surfaced as a keep-alive `Message` with empty values but full ts/period/seq metadata.
- **Encode** is raw passthrough — bytes go out unchanged. Use the engine's `send_raw` command to trigger the MCU stream.

"#
        .to_string()
    }
}
