use crc::Crc;
use cycbox_sdk::prelude::*;

/// "CBRT" sync word.
pub const CBRT_SYNC: [u8; 4] = [0x43, 0x42, 0x52, 0x54];

/// CRC-16/MODBUS — poly 0x8005, init 0xFFFF, input/output reflected, no final XOR.
/// Same algorithm as modbus-codec (see `crates/modbus-codec/src/modbus_rtu/crc.rs`).
const CRC16: Crc<u16> = Crc::<u16>::new(&crc::CRC_16_MODBUS);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SessionProfile {
    /// High nibble of byte at offset 4 (`has_ts | has_period | has_crc | has_seq`).
    flag_bits: u8,
    /// Datatype code (low nibble).
    datatype: u8,
    /// 1..=64.
    channels: u8,
}

/// Per-session decode state. One instance per logical CBRT session
/// (stream codec: one; message-based transformer: one per connection_id).
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    profile: Option<SessionProfile>,
    last_seq: Option<u8>,
    last_ts_raw: Option<u32>,
    wrap_count: u64,
    frame_count: u64,
    /// Host-to-device clock offset, tracked as `min(arrival - ts)` across the session so
    /// frames that happened to arrive with low transport delay tighten the anchor toward
    /// the true one-way latency floor. `base_us = full_ts_us as i64 + ts_anchor_us`.
    ts_anchor_us: Option<i64>,
    /// Anchor timestamp of the previous frame, used to infer the sample period when a
    /// frame carries no `period` field. For `has_ts` frames this is the first sample's
    /// time; for frames without `has_ts` it is the last sample's time (== arrival).
    last_anchor_us: Option<u64>,
    /// Sample count of the previous frame. Needed to infer the period for `has_ts`
    /// frames, where the inter-anchor gap spans the *previous* frame's samples.
    last_sample_count: Option<usize>,
    /// EMA-smoothed inferred period in microseconds (fractional), used when the frame
    /// omits the `period` field.
    period_ema_us: Option<f64>,
}

impl SessionState {
    pub fn reset(&mut self) {
        self.profile = None;
        self.last_seq = None;
        self.last_ts_raw = None;
        self.wrap_count = 0;
        self.frame_count = 0;
        self.ts_anchor_us = None;
        self.last_anchor_us = None;
        self.last_sample_count = None;
        self.period_ema_us = None;
    }
}

pub enum ParseOutcome {
    Complete { frame_end: usize, message: Message },
    NeedMore,
    Reject,
}

enum SeqEvent {
    Duplicate,
    Dropped(u8),
}

pub fn parse_at(
    state: &mut SessionState,
    buf: &[u8],
    sync_pos: usize,
    arrival_us: u64,
) -> ParseOutcome {
    // Need sync(4) + flags(1) + channels(1) at minimum.
    const MIN_HEAD: usize = 6;
    if buf.len() < sync_pos + MIN_HEAD {
        return ParseOutcome::NeedMore;
    }

    let flags_byte = buf[sync_pos + 4];
    let has_ts = flags_byte & 0b1000_0000 != 0;
    let has_period = flags_byte & 0b0100_0000 != 0;
    let has_crc = flags_byte & 0b0010_0000 != 0;
    let has_seq = flags_byte & 0b0001_0000 != 0;
    let datatype = flags_byte & 0x0F;

    if datatype == 0xF {
        return ParseOutcome::Reject;
    }

    let channels = buf[sync_pos + 5];
    if channels == 0 || channels > 64 {
        return ParseOutcome::Reject;
    }

    let mut cur = sync_pos + 6;

    let seq = if has_seq {
        if buf.len() < cur + 1 {
            return ParseOutcome::NeedMore;
        }
        let v = buf[cur];
        cur += 1;
        Some(v)
    } else {
        None
    };

    let ts_raw = if has_ts {
        if buf.len() < cur + 4 {
            return ParseOutcome::NeedMore;
        }
        let v = u32::from_le_bytes([buf[cur], buf[cur + 1], buf[cur + 2], buf[cur + 3]]);
        cur += 4;
        Some(v)
    } else {
        None
    };

    let period_us = if has_period {
        if buf.len() < cur + 2 {
            return ParseOutcome::NeedMore;
        }
        let v = u16::from_le_bytes([buf[cur], buf[cur + 1]]);
        cur += 2;
        Some(v)
    } else {
        None
    };

    if buf.len() < cur + 2 {
        return ParseOutcome::NeedMore;
    }
    let payload_len = u16::from_le_bytes([buf[cur], buf[cur + 1]]) as usize;
    cur += 2;

    // Exactness check (§6.1.6).
    let chans = channels as usize;
    if datatype == 0xE {
        if (payload_len * 8) % chans != 0 {
            return ParseOutcome::Reject;
        }
    } else {
        let bps = match bytes_per_datatype(datatype) {
            Some(v) => v,
            None => return ParseOutcome::Reject,
        };
        if payload_len % (chans * bps) != 0 {
            return ParseOutcome::Reject;
        }
    }

    if buf.len() < cur + payload_len {
        return ParseOutcome::NeedMore;
    }
    let payload_start = cur;
    let payload_end = cur + payload_len;
    cur = payload_end;

    let frame_end = if has_crc {
        if buf.len() < cur + 2 {
            return ParseOutcome::NeedMore;
        }
        let stored = u16::from_le_bytes([buf[cur], buf[cur + 1]]);
        // CRC covers offset 4 (flags byte) through end of payload, sync excluded (§3.9).
        let computed = CRC16.checksum(&buf[sync_pos + 4..payload_end]);
        if computed != stored {
            log::debug!(
                "cbrt: CRC mismatch at sync_pos={} (expected 0x{:04X}, got 0x{:04X}); resync",
                sync_pos,
                computed,
                stored
            );
            return ParseOutcome::Reject;
        }
        cur + 2
    } else {
        cur
    };

    // Session profile (§5.2 / §6.3): first valid frame defines the profile;
    // any subsequent change starts a new session.
    let new_profile = SessionProfile {
        flag_bits: flags_byte & 0xF0,
        datatype,
        channels,
    };
    let session_changed = match state.profile {
        None => true,
        Some(p) => p != new_profile,
    };
    if session_changed {
        state.profile = Some(new_profile);
        state.last_seq = None;
        state.last_ts_raw = None;
        state.wrap_count = 0;
        state.frame_count = 0;
        state.ts_anchor_us = None;
        state.last_anchor_us = None;
        state.last_sample_count = None;
        state.period_ema_us = None;
    }

    // Wrap detection (§6.4).
    let full_ts_us = ts_raw.map(|raw| {
        if let Some(prev) = state.last_ts_raw
            && raw < prev
            && (prev - raw) > 0x8000_0000
        {
            state.wrap_count = state.wrap_count.saturating_add(1);
        }
        state.last_ts_raw = Some(raw);
        (state.wrap_count << 32) | (raw as u64)
    });

    // Sequence drop detection (§6.6).
    let dropped = match (seq, state.last_seq) {
        (Some(cur_s), Some(last)) => {
            let delta = cur_s.wrapping_sub(last);
            if delta == 0 {
                Some(SeqEvent::Duplicate)
            } else if delta > 1 {
                Some(SeqEvent::Dropped(delta - 1))
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(s) = seq {
        state.last_seq = Some(s);
    }

    // Anchor per-sample timestamps to the device clock when `has_ts` is present, so
    // hostside transport/scheduling jitter doesn't show up between frames. The MCU's
    // `ts` field is the time of the first sample in the frame (§6.5). We track the
    // host/device offset as `min(arrival - ts)` across the session — a frame with
    // low transport delay ratchets the anchor down toward the true one-way latency
    // floor, so a slow first frame (e.g. TCP handshake/buffering) doesn't poison
    // every subsequent sample.
    let base_us = match full_ts_us {
        Some(ts) => {
            let candidate = (arrival_us as i64).saturating_sub(ts as i64);
            let anchor = match state.ts_anchor_us {
                Some(prev) => prev.min(candidate),
                None => candidate,
            };
            state.ts_anchor_us = Some(anchor);
            (ts as i64).saturating_add(anchor).max(0) as u64
        }
        None => arrival_us,
    };

    let payload_slice = &buf[payload_start..payload_end];
    let sample_count = compute_sample_count(payload_len, datatype, channels);

    // Resolve the per-sample timing for this frame.
    //
    // Anchor semantics (§6.5): when `has_ts` is present the device timestamp is the time
    // of the FIRST sample, so samples spread forward from `base_us`. When it is absent
    // the host arrival time is taken as the time of the LAST sample, so samples spread
    // backward from `base_us`.
    //
    // Period: an explicit `period` field always wins. Otherwise we infer it from the
    // spacing of consecutive frame anchors and the number of samples spanning the gap,
    // EMA-smoothed across frames to absorb jitter. This is a lag-1 estimate — the current
    // frame is spaced with the most recently measured rate — so the first frame of a
    // session (no reference yet) collapses its samples onto the anchor.
    let provided_period = period_us.map(|p| p as f64).filter(|p| *p > 0.0);
    let effective_period = match provided_period {
        Some(p) => Some(p),
        None => infer_period(state, has_ts, base_us, sample_count),
    };
    let timing = SampleTiming {
        anchor_us: base_us,
        period_us: effective_period,
        anchor_at_last: !has_ts,
        sample_count,
    };
    let values = materialize_values(datatype, channels, payload_slice, &timing);

    let frame_slice = &buf[sync_pos..frame_end];
    let contents = format_contents(
        frame_slice,
        has_seq,
        has_ts,
        has_period,
        has_crc,
        payload_len,
    );

    let mut metadata = Vec::with_capacity(4);
    metadata.push(Value::builder("datatype").string(datatype_name(datatype)));
    metadata.push(Value::builder("samples").uint32(sample_count as u32));
    if let Some(p) = effective_period {
        metadata.push(Value::builder("period_us").uint32(p.round().min(u32::MAX as f64) as u32));
    }
    match dropped {
        Some(SeqEvent::Duplicate) => {
            metadata.push(Value::builder("seq_duplicate").boolean(true));
        }
        Some(SeqEvent::Dropped(n)) => {
            metadata.push(Value::builder("seq_dropped").uint8(n));
        }
        None => {}
    }
    state.frame_count = state.frame_count.saturating_add(1);

    let message = MessageBuilder::rx(
        PayloadType::Binary,
        payload_slice.to_vec(),
        frame_slice.to_vec(),
    )
    .timestamp(arrival_us)
    .add_values(values)
    .metadata(metadata)
    .contents(contents)
    .build();

    ParseOutcome::Complete { frame_end, message }
}

fn bytes_per_datatype(dt: u8) -> Option<usize> {
    Some(match dt {
        0x0 | 0x1 => 1,
        0x2 | 0x3 | 0xA | 0xC | 0xD => 2,
        0x4 | 0x5 | 0x8 | 0xB => 4,
        0x6 | 0x7 | 0x9 => 8,
        _ => return None, // 0xE (bool-packed) and 0xF (reserved) handled by caller.
    })
}

fn datatype_name(dt: u8) -> &'static str {
    match dt {
        0x0 => "u8",
        0x1 => "i8",
        0x2 => "u16",
        0x3 => "i16",
        0x4 => "u32",
        0x5 => "i32",
        0x6 => "u64",
        0x7 => "i64",
        0x8 => "f32",
        0x9 => "f64",
        0xA => "q15",
        0xB => "q31",
        0xC => "bf16",
        0xD => "f16",
        0xE => "bool",
        _ => "unknown",
    }
}

fn compute_sample_count(payload_len: usize, datatype: u8, channels: u8) -> usize {
    if channels == 0 {
        return 0;
    }
    let chans = channels as usize;
    if datatype == 0xE {
        (payload_len * 8) / chans
    } else if let Some(bps) = bytes_per_datatype(datatype) {
        payload_len / (chans * bps)
    } else {
        0
    }
}

/// Resolved per-sample timing for a single frame.
struct SampleTiming {
    /// For `has_ts` frames, the time of the first sample; otherwise the time of the last
    /// sample (host arrival).
    anchor_us: u64,
    /// Sample period in microseconds (fractional). `None` means the period is unknown
    /// (first frame of a session with no explicit `period` field), in which case all
    /// samples collapse onto the anchor.
    period_us: Option<f64>,
    /// When true the anchor is the last sample and samples spread backward in time; when
    /// false the anchor is the first sample and samples spread forward.
    anchor_at_last: bool,
    sample_count: usize,
}

impl SampleTiming {
    /// Timestamp (µs) of sample `i`, counted from the anchor according to the anchor
    /// semantics and period. With no period, every sample shares the anchor.
    fn ts(&self, i: usize) -> u64 {
        match self.period_us {
            Some(p) => {
                let k = if self.anchor_at_last {
                    // Anchor is the last sample: sample i sits (count-1-i) periods earlier.
                    i as f64 - (self.sample_count.max(1) as f64 - 1.0)
                } else {
                    i as f64
                };
                let t = self.anchor_us as f64 + k * p;
                if t <= 0.0 { 0 } else { t.round() as u64 }
            }
            None => self.anchor_us,
        }
    }
}

/// EMA smoothing factor for inferred-period tracking. Small enough to absorb host
/// arrival jitter, large enough to follow genuine rate changes within a few frames.
const PERIOD_EMA_ALPHA: f64 = 0.2;

/// Infer the sample period (µs) from the spacing of consecutive frame anchors when the
/// frame carries no explicit `period` field, updating the session's EMA state.
///
/// The sample count spanning the inter-anchor gap depends on the anchor semantics: for
/// `has_ts` frames the anchor is the first sample, so the gap from the previous anchor to
/// this one spans the *previous* frame's samples; for frames without `has_ts` the anchor
/// is the last sample, so the gap spans the *current* frame's samples.
///
/// Returns the current EMA estimate (the lag-1 rate used to space this frame), or `None`
/// for the first frame of a session, where no rate is known yet.
fn infer_period(
    state: &mut SessionState,
    has_ts: bool,
    anchor_us: u64,
    sample_count: usize,
) -> Option<f64> {
    if let Some(last_anchor) = state.last_anchor_us
        && anchor_us > last_anchor
    {
        let span = if has_ts {
            state.last_sample_count.unwrap_or(0)
        } else {
            sample_count
        };
        if span > 0 {
            let raw = (anchor_us - last_anchor) as f64 / span as f64;
            if raw.is_finite() && raw > 0.0 {
                state.period_ema_us = Some(match state.period_ema_us {
                    Some(prev) => PERIOD_EMA_ALPHA * raw + (1.0 - PERIOD_EMA_ALPHA) * prev,
                    None => raw,
                });
            }
        }
    }
    state.last_anchor_us = Some(anchor_us);
    state.last_sample_count = Some(sample_count);
    state.period_ema_us
}

fn materialize_values(
    datatype: u8,
    channels: u8,
    payload: &[u8],
    timing: &SampleTiming,
) -> Vec<Value> {
    let chans = channels as usize;
    let sample_count = timing.sample_count;
    let mut out = Vec::with_capacity(sample_count * chans);

    if datatype == 0xE {
        for s in 0..sample_count {
            let ts = timing.ts(s);
            for ch in 0..chans {
                let bit_idx = s * chans + ch;
                let byte = payload[bit_idx / 8];
                let bit = (byte >> (7 - (bit_idx % 8))) & 0x1;
                out.push(
                    Value::builder(channel_id(ch))
                        .timestamp(ts)
                        .boolean(bit != 0),
                );
            }
        }
        return out;
    }

    let bps = match bytes_per_datatype(datatype) {
        Some(v) => v,
        None => return out,
    };
    let row = bps * chans;

    for s in 0..sample_count {
        let ts = timing.ts(s);
        for ch in 0..chans {
            let off = s * row + ch * bps;
            let id = channel_id(ch);
            let v = match datatype {
                0x0 => Value::builder(id).timestamp(ts).uint8(payload[off]),
                0x1 => Value::builder(id).timestamp(ts).int8(payload[off] as i8),
                0x2 => Value::builder(id)
                    .timestamp(ts)
                    .uint16(u16::from_le_bytes([payload[off], payload[off + 1]])),
                0x3 => Value::builder(id)
                    .timestamp(ts)
                    .int16(i16::from_le_bytes([payload[off], payload[off + 1]])),
                0x4 => Value::builder(id).timestamp(ts).uint32(u32::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                ])),
                0x5 => Value::builder(id).timestamp(ts).int32(i32::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                ])),
                0x6 => Value::builder(id).timestamp(ts).uint64(u64::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                    payload[off + 4],
                    payload[off + 5],
                    payload[off + 6],
                    payload[off + 7],
                ])),
                0x7 => Value::builder(id).timestamp(ts).int64(i64::from_le_bytes([
                    payload[off],
                    payload[off + 1],
                    payload[off + 2],
                    payload[off + 3],
                    payload[off + 4],
                    payload[off + 5],
                    payload[off + 6],
                    payload[off + 7],
                ])),
                0x8 => Value::builder(id)
                    .timestamp(ts)
                    .float32(f32::from_le_bytes([
                        payload[off],
                        payload[off + 1],
                        payload[off + 2],
                        payload[off + 3],
                    ])),
                0x9 => Value::builder(id)
                    .timestamp(ts)
                    .float64(f64::from_le_bytes([
                        payload[off],
                        payload[off + 1],
                        payload[off + 2],
                        payload[off + 3],
                        payload[off + 4],
                        payload[off + 5],
                        payload[off + 6],
                        payload[off + 7],
                    ])),
                0xA => {
                    let raw = i16::from_le_bytes([payload[off], payload[off + 1]]);
                    Value::builder(id)
                        .timestamp(ts)
                        .float32(raw as f32 / 32768.0)
                }
                0xB => {
                    let raw = i32::from_le_bytes([
                        payload[off],
                        payload[off + 1],
                        payload[off + 2],
                        payload[off + 3],
                    ]);
                    Value::builder(id)
                        .timestamp(ts)
                        .float32(raw as f32 / 2_147_483_648.0)
                }
                0xC => {
                    let bits = u16::from_le_bytes([payload[off], payload[off + 1]]);
                    Value::builder(id).timestamp(ts).float32(bf16_to_f32(bits))
                }
                0xD => {
                    let bits = u16::from_le_bytes([payload[off], payload[off + 1]]);
                    Value::builder(id).timestamp(ts).float32(f16_to_f32(bits))
                }
                _ => continue,
            };
            out.push(v);
        }
    }

    out
}

fn channel_id(ch: usize) -> String {
    format!("ch{ch}")
}

fn bf16_to_f32(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 0x1) as u32;
    let exp = ((bits >> 10) & 0x1F) as u32;
    let mant = (bits & 0x3FF) as u32;
    let out_bits: u32 = match exp {
        0 => {
            if mant == 0 {
                sign << 31
            } else {
                // Subnormal — normalize.
                let mut m = mant;
                let mut e: i32 = -14;
                while m & 0x400 == 0 {
                    m <<= 1;
                    e -= 1;
                }
                m &= 0x3FF;
                let e_out = (e + 127) as u32;
                (sign << 31) | (e_out << 23) | (m << 13)
            }
        }
        0x1F => (sign << 31) | (0xFF << 23) | (mant << 13),
        _ => {
            let e_out = (exp as i32 - 15 + 127) as u32;
            (sign << 31) | (e_out << 23) | (mant << 13)
        }
    };
    f32::from_bits(out_bits)
}

fn format_contents(
    frame: &[u8],
    has_seq: bool,
    has_ts: bool,
    has_period: bool,
    has_crc: bool,
    payload_len: usize,
) -> Vec<Content> {
    let mut contents = Vec::new();
    let mut pos = 0;
    let push_hex = |contents: &mut Vec<Content>,
                    frame: &[u8],
                    pos: &mut usize,
                    n: usize,
                    label: &str,
                    kind: ContentKind| {
        let end = (*pos + n).min(frame.len());
        let hex = hex_of(&frame[*pos..end]);
        let label_owned = Some(label.to_string());
        let c = match kind {
            ContentKind::Header => Content::header(hex.into_bytes(), label_owned),
            ContentKind::Length => Content::length_field(hex.into_bytes(), label_owned),
            ContentKind::Data => Content::data(hex.into_bytes(), label_owned),
            ContentKind::Checksum => Content::checksum(hex.into_bytes(), label_owned),
        };
        contents.push(c);
        *pos += n;
    };

    push_hex(
        &mut contents,
        frame,
        &mut pos,
        4,
        "Sync",
        ContentKind::Header,
    );
    push_hex(&mut contents, frame, &mut pos, 1, "FD", ContentKind::Header);
    push_hex(&mut contents, frame, &mut pos, 1, "CH", ContentKind::Header);
    if has_seq {
        push_hex(
            &mut contents,
            frame,
            &mut pos,
            1,
            "Seq",
            ContentKind::Header,
        );
    }
    if has_ts {
        push_hex(
            &mut contents,
            frame,
            &mut pos,
            4,
            "Timestamp us",
            ContentKind::Header,
        );
    }
    if has_period {
        push_hex(
            &mut contents,
            frame,
            &mut pos,
            2,
            "Period us",
            ContentKind::Header,
        );
    }
    push_hex(
        &mut contents,
        frame,
        &mut pos,
        2,
        "Len",
        ContentKind::Length,
    );
    if payload_len > 0 {
        push_hex(
            &mut contents,
            frame,
            &mut pos,
            payload_len,
            "Payload",
            ContentKind::Data,
        );
    }
    if has_crc {
        push_hex(
            &mut contents,
            frame,
            &mut pos,
            2,
            "CRC-16",
            ContentKind::Checksum,
        );
    }

    contents
}

enum ContentKind {
    Header,
    Length,
    Data,
    Checksum,
}

fn hex_of(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 3);
    for (i, byte) in b.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{byte:02X}"));
    }
    s
}
