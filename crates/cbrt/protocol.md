# CBRT — CycBox Realtime Protocol

A compact, length-delimited binary protocol for streaming multi-channel realtime sensor / signal data over serial, TCP, UDP, BLE, and similar transports. Optimized for embedded / IoT: small overhead, single-pass decode, robust resync, optional integrity check.

---

## 1. Conventions

- **Endianness**: All multi-byte fields, and all multi-byte samples within the payload, are **little-endian**.
- **Bit numbering**: bit 7 = MSB, bit 0 = LSB, within a byte.
- **Hex notation**: `0xNN` for a single byte; bytes given left-to-right in wire order.
- **MUST / SHOULD / MAY** follow RFC 2119 usage.
- A **frame** is one self-contained protocol unit. A **session** is the (logical) stream of frames between an encoder and a decoder, beginning with the first valid frame received.

---

## 2. Frame layout

```
Offset  Size  Field
─────────────────────────────────────────────────────────────
0       4     Sync word                    "CBRT" = 43 42 52 54
4       1     Flags (high nibble) + Datatype (low nibble)
              bit 7: has_ts        bit 3..0: datatype code
              bit 6: has_period
              bit 5: has_crc
              bit 4: has_seq
5       1     Channel count                1..64; else invalid
[?      1]    Sequence number              present iff has_seq=1
[?      4]    Timestamp µs (u32 LE)        present iff has_ts=1
[?      2]    Sample period µs (u16 LE)    present iff has_period=1
?       2     Payload length (u16 LE)      bytes of payload only
?       N     Payload                      channel-interleaved, LE
[?      2]    CRC-16/MODBUS (LE)           present iff has_crc=1
```

Notes:
- Optional fields appear in the order shown above. Because four fields are flag-gated, `payload_length` sits at a variable offset between 8 and 14. Decoders MUST track a running cursor rather than hard-code offsets.
- The 4 lower bits of `flags` byte are part of the datatype code table (§4).

### 2.1 Overhead table

| Configuration            | Header | Footer | Total fixed |
| ------------------------ | ------ | ------ | ----------- |
| Bare (no optional flags) | 8 B    | 0      | 8 B         |
| `has_seq + has_crc`      | 9 B    | 2 B    | 11 B        |
| `has_ts` (one-shot)      | 12 B   | 0      | 12 B        |
| All flags set            | 14 B   | 2 B    | 16 B        |

---

## 3. Field semantics

### 3.1 Sync word — `"CBRT"`
The four ASCII bytes `0x43 0x42 0x52 0x54` ("CBRT" = CycBox Real-Time). Used for frame boundary detection. The sync word is **excluded** from CRC computation; integrity of the sync is established by the validation chain in §6.

### 3.2 Flags + datatype byte (offset 4)

| Bit  | Name         | Meaning                                                        |
| ---- | ------------ | -------------------------------------------------------------- |
| 7    | `has_ts`     | If 1, a 4-byte timestamp follows the optional `seq` field.     |
| 6    | `has_period` | If 1, a 2-byte sample period follows the optional `ts` field.  |
| 5    | `has_crc`    | If 1, a 2-byte CRC trails the payload.                         |
| 4    | `has_seq`    | If 1, a 1-byte sequence number follows the channel-count byte. |
| 3..0 | `datatype`   | One of the codes in §4. Code `0xF` is reserved and invalid.    |

### 3.3 Channel count (offset 5)
Number of channels per sample. Range `1..=64`. Values `0` or `65..=255` are invalid and MUST cause the candidate frame to be rejected.

### 3.4 Sequence number (optional, 1 B)
Increments by 1 modulo 256 per frame sent on a session. The first frame's value is implementation-defined (typically `0`). Decoders use it to detect drops on lossy transports.

### 3.5 Timestamp µs (optional, u32 LE)
Microseconds since an encoder-defined session epoch (typically `0` at session start, or device boot time). The 32-bit field wraps every `2^32 µs ≈ 71.58 min` — see §6.4 for wrap handling.

The timestamp denotes the sampling instant of the **first sample** in the frame.

### 3.6 Sample period µs (optional, u16 LE)
The µs interval between consecutive samples within (and between) frames. Range `1..=65535` µs (1 MHz down to ~15 Hz). For rates outside this range, omit `has_period` and convey timing via `has_ts` only.

### 3.7 Payload length (u16 LE)
Length of the payload in bytes. Range `0..=65535`. A frame with `payload_length=0` is a valid keep-alive (typically combined with `has_ts=1` and/or `has_period=1` to anchor timing).

Transports SHOULD impose a maximum-frame-payload cap suitable to their MTU and latency budget (suggested: 4 KiB on BLE, 16 KiB on TCP/serial).

### 3.8 Payload
A tightly-packed, channel-interleaved array of samples in the declared datatype:
```
ch0_s0, ch1_s0, …, chN-1_s0, ch0_s1, ch1_s1, …, chN-1_s1, …
```
For fixed-width datatypes:
```
sample_count = payload_length / (channel_count × bytes_per_datatype)
```
This division MUST be exact; otherwise the frame is invalid.

For `0xE bool-packed`:
- Samples form a single channel-interleaved bitstream.
- MSB-first within each byte (bit 7 carries the earliest channel of the earliest sample of that byte).
- The bitstream is zero-padded **only at the end of the payload** to reach a whole-byte boundary; no per-sample padding.
- `sample_count = (payload_length × 8) / channel_count`; this division MUST be exact (the encoder pads with whole zero samples if necessary to align).

### 3.9 CRC-16/MODBUS (optional, u16 LE)
Polynomial `0x8005`, init `0xFFFF`, input reflected, output reflected, no final XOR. This is the same algorithm used by Modbus RTU. Computed over **offset 4 through the last byte of the payload, inclusive** (i.e., flags+datatype byte, channel count, all present optional header fields, payload length, and payload). The sync word is not included.

Reference value: CRC-16/MODBUS of the ASCII string `"123456789"` is `0x4B37`.

---

## 4. Datatype table

| Code  | Name          | Bytes/sample | Range / Semantics                                                        |
| ----- | ------------- | ------------ | ------------------------------------------------------------------------ |
| `0x0` | `u8`          | 1            | unsigned 8-bit                                                           |
| `0x1` | `i8`          | 1            | signed 8-bit, two's complement                                           |
| `0x2` | `u16`         | 2            | unsigned 16-bit, little-endian                                           |
| `0x3` | `i16`         | 2            | signed 16-bit, little-endian                                             |
| `0x4` | `u32`         | 4            | unsigned 32-bit, little-endian                                           |
| `0x5` | `i32`         | 4            | signed 32-bit, little-endian                                             |
| `0x6` | `u64`         | 8            | unsigned 64-bit, little-endian                                           |
| `0x7` | `i64`         | 8            | signed 64-bit, little-endian                                             |
| `0x8` | `f32`         | 4            | IEEE 754 binary32, little-endian                                         |
| `0x9` | `f64`         | 8            | IEEE 754 binary64, little-endian                                         |
| `0xA` | `q15`         | 2            | i16 fixed-point: real value = `raw / 32768.0`, range `[-1.0, +1.0)`      |
| `0xB` | `q31`         | 4            | i32 fixed-point: real value = `raw / 2147483648.0`, range `[-1.0, +1.0)` |
| `0xC` | `bf16`        | 2            | bfloat16 (1 sign / 8 exp / 7 mantissa), little-endian                    |
| `0xD` | `f16`         | 2            | IEEE 754 binary16 (1 / 5 / 10), little-endian                            |
| `0xE` | `bool-packed` | —            | 1 bit / sample, MSB-first, channel-interleaved (see §3.8)                |
| `0xF` | *reserved*    | —            | invalid; decoder MUST reject                                             |

All channels within a single frame share the same datatype.

---

## 5. Encoder rules

5.1 The encoder MUST emit the sync word followed by the flags+datatype byte followed by the channel-count byte, then each optional header field that its corresponding flag enables, in the order defined in §2, then `payload_length`, then `payload`, then the optional CRC.

5.2 **Session flag stability.** The set of flag bits (`has_ts`, `has_period`, `has_seq`, `has_crc`) chosen for the **first frame** of a session is the session's flag profile. The encoder MUST emit every subsequent frame with the same flag profile and the same datatype code and same channel count. To change any of these, the encoder MUST start a new session (e.g., reconnect, or signal a session boundary out-of-band).

5.3 **Timestamped sessions** (`has_ts=1` in frame 1): each frame's timestamp denotes the sampling instant of its first sample. The encoder MAY also set `has_period=1` to convey sample rate. If `has_period=1`, the period field is repeated on every frame (the value MAY change between frames if the rate actually changes).

5.4 **Untimestamped sessions** (`has_ts=0` in frame 1): intended for slow / sparse / asynchronous samples where µs-precision sampling instants are not required. The decoder will assign each frame a timestamp based on its receive time (§6.5).

5.5 If `has_seq=1`, the encoder increments the sequence number by 1 (mod 256) for each frame sent.

5.6 If `has_crc=1`, the encoder computes the CRC over offset 4 through end-of-payload as specified in §3.9 and appends it.

5.7 Encoders SHOULD emit at least one frame every ~30 minutes on a timestamped session, even if it is a `payload_length=0` keep-alive, to keep the decoder's wrap counter unambiguous (§6.4).

5.8 Encoders SHOULD respect a transport-appropriate maximum-frame-payload cap (§3.7).

---

## 6. Decoder rules

### 6.1 Frame validation chain
For a candidate frame to be accepted, the decoder MUST verify, in order:
1. The four bytes at the current cursor equal `0x43 0x42 0x52 0x54` ("CBRT").
2. `datatype != 0xF` (reserved).
3. `1 ≤ channel_count ≤ 64`.
4. Reading the optional header fields and `payload_length` does not exceed the available input.
5. `payload_length ≤ MAX_FRAME_PAYLOAD` (transport-configured).
6. For fixed-width datatypes: `payload_length mod (channel_count × bytes_per_datatype) == 0`.
   For `0xE bool-packed`: `(payload_length × 8) mod channel_count == 0`.
7. If `has_crc=1`: the trailing CRC matches the CRC computed over offset 4 through end-of-payload.

If any check fails, the candidate frame is rejected (see §6.2).

### 6.2 Resync algorithm
On rejection, the decoder MUST:
1. Advance the input cursor by **1 byte** past the start of the rejected candidate.
2. Scan forward for the next occurrence of the sync word.
3. Restart the validation chain at that position.

Decoders MUST NOT skip more than one byte at a time on rejection; this guarantees recovery from any single bit-flip with at most one frame's data loss.

### 6.3 Session establishment
The decoder treats the **first frame that passes validation** as defining the session's flag profile, datatype, and channel count. Any subsequent frame that passes individual validation but has a differing flag profile, datatype, or channel count MUST be treated as the start of a **new session** (and any in-progress decode state — wrap counter, last sequence number, last period — reset).

### 6.4 Timestamp wrap (timestamped sessions)
The 32-bit µs timestamp wraps every `2^32 µs ≈ 71.58 minutes`. Decoders MUST maintain a 64-bit wrap counter:

```
let raw       = current_frame.ts_us;      // u32
let prev_raw  = last_ts_raw;              // u32, from previous frame
if raw < prev_raw && (prev_raw - raw) > 0x80000000 {
    wrap_count += 1;
}
let full_ts_us = ((wrap_count as u64) << 32) | (raw as u64);
last_ts_raw = raw;
```

If the gap between two consecutive frames exceeds ~35 minutes of wall time, wrap detection is **ambiguous**: the decoder cannot know whether the timestamp wrapped or simply jumped forward. Encoders avoid this by emitting periodic keep-alives (§5.7). Decoders SHOULD log a warning and continue with best-effort wrap detection.

### 6.5 Untimestamped sessions
If `has_ts=0` for the session, the decoder assigns each frame a timestamp equal to its **arrival time** (decoder wall-clock at the moment the frame's last byte is read). Intended for slow or sparse data where source-side µs precision is unnecessary; not suitable for jitter-sensitive analysis.

If `has_period=0` as well, sample-rate information is unavailable; consumers MUST treat samples as individually-timed at the frame arrival instant or rely on application-level rate knowledge.

### 6.6 Sequence-number drop detection
If `has_seq=1`, the decoder computes `delta = (current_seq - last_seq) mod 256`. `delta == 1` indicates in-order delivery; `delta > 1` indicates `delta - 1` dropped frames; `delta == 0` indicates a duplicate (possible on retransmitting transports). Consumers MAY surface this as a quality signal.

### 6.7 Sample materialization
After validation, samples are materialized from the payload as a 2-D array `[sample_count][channel_count]` of the declared datatype. Each fixed-width sample is read as a little-endian integer or IEEE float. Q15 and Q31 samples are read as signed integers and divided by `32768.0` and `2147483648.0` respectively to produce real values in `[-1.0, +1.0)`. Bool-packed samples are unpacked MSB-first per §3.8.

### 6.8 Per-sample timestamping
For a timestamped frame with `has_period=1`, the timestamp of sample `i` (0-indexed) is `ts_us + i × period_us`. Without `has_period`, only the first sample's timestamp is known precisely; consumers that need per-sample timestamps SHOULD require `has_period=1` upstream.

---

## 7. Versioning

The protocol version is identified by the sync word. Future incompatible versions MUST use a different sync word (e.g., `"CBR2"` = `0x43 0x42 0x52 0x32`). A decoder MAY support multiple versions concurrently by selecting parsing logic based on the matched sync word.

Within version 1 (`"CBRT"`), the four currently-unused datatype codes (`0xC`, `0xD`, `0xE` are allocated above; `0xF` reserved) and any future minor extensions reuse existing flag-gated optional-field slots; no flag bits are currently free, so any field-shape change requires a new version.

---

## 8. Worked example

**Scenario**: 4 channels of `i16` (e.g., a 4-axis IMU's raw accelerometer counts), 10 samples per frame, 1 ms sample period, full optional fields enabled, frame timestamp = 1,000,000 µs, sequence number = 0.

**Field-by-field encoding**:

| Field                       | Bytes (hex)   | Notes                                                             |
| --------------------------- | ------------- | ----------------------------------------------------------------- |
| Sync `"CBRT"`               | `43 42 52 54` |                                                                   |
| Flags + datatype            | `F3`          | bits 7..4 = `1111` (all flags), bits 3..0 = `0011` (i16)          |
| Channel count               | `04`          | 4 channels                                                        |
| Sequence number             | `00`          | first frame                                                       |
| Timestamp µs (u32 LE)       | `40 42 0F 00` | `0x000F4240` = 1,000,000                                          |
| Sample period µs (u16 LE)   | `E8 03`       | `0x03E8` = 1000 µs                                                |
| Payload length (u16 LE)     | `50 00`       | `0x0050` = 80 bytes (4 ch × 2 B × 10 samples)                     |
| Payload                     | 80 bytes      | interleaved i16 LE: `[s0_ch0, s0_ch1, s0_ch2, s0_ch3, s1_ch0, …]` |
| CRC-16/MODBUS (u16 LE)      | `?? ??`       | over the 95 bytes from offset 4 through end-of-payload            |

**Total frame length**: `4 + 1 + 1 + 1 + 4 + 2 + 2 + 80 + 2 = 97 bytes`.

For the same data without any optional flags (`has_ts=has_period=has_seq=has_crc=0`):

| Field                     | Bytes         |
| ------------------------- | ------------- |
| Sync                      | `43 42 52 54` |
| Flags + datatype (`0x03`) | `03`          |
| Channel count             | `04`          |
| Payload length            | `50 00`       |
| Payload                   | 80 bytes      |

**Total**: `8 + 80 = 88 bytes`. Header overhead drops to 10%.

---

## 9. Edge cases & rejection conditions (summary)

| Condition                                                                                                                               | Outcome                                                                    |
| --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Sync mismatch                                                                                                                           | Advance 1 byte, rescan                                                     |
| `datatype == 0xF`                                                                                                                       | Reject, resync                                                             |
| `channel_count == 0` or `> 64`                                                                                                          | Reject, resync                                                             |
| `payload_length > MAX_FRAME_PAYLOAD`                                                                                                    | Reject, resync                                                             |
| `payload_length` not a multiple of `channels × bytes_per_sample` (or, for bool-packed, `payload_length × 8` not a multiple of channels) | Reject, resync                                                             |
| Insufficient bytes available to complete frame                                                                                          | Wait for more input (do not advance)                                       |
| CRC mismatch (when `has_crc=1`)                                                                                                         | Reject, advance 1 byte, resync                                             |
| Flag profile / datatype / channel-count changes mid-session                                                                             | Treat as new session: reset wrap counter, last-seq, last-period            |
| `payload_length == 0`                                                                                                                   | Valid (keep-alive); produce zero samples; still updates ts/period state    |
| Timestamp appears to go backwards by less than `2^31 µs`                                                                                | Treat as out-of-order or jitter, not wrap; consumer's call whether to drop |
| Timestamp appears to go backwards by more than `2^31 µs`                                                                                | Treat as 32-bit wrap (§6.4)                                                |
| Gap between frames exceeds ~35 min on a timestamped session                                                                             | Wrap detection ambiguous; warn                                             |

---

## 10. Test vectors (to be filled in by the reference implementation)

Implementations SHOULD validate against a shared set of golden frames covering: each datatype, with and without each optional flag, including a deliberately-corrupted variant of each to exercise the resync path. Test vectors live at `crates/<codec-crate>/tests/vectors/`.

---

