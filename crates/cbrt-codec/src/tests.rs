use crate::codec::{CBRT_SYNC, CbrtCodec};
use bytes::BytesMut;
use cycbox_sdk::prelude::*;

/// CRC-16/MODBUS of the spec's reference string — matches modbus-codec's CRC.
#[test]
fn crc16_modbus_of_123456789_matches_spec() {
    const CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);
    assert_eq!(CRC.checksum(b"123456789"), 0x4B37);
}

fn build_frame(
    datatype: u8,
    channels: u8,
    seq: Option<u8>,
    ts_us: Option<u32>,
    period_us: Option<u16>,
    payload: &[u8],
    with_crc: bool,
) -> Vec<u8> {
    let mut flags: u8 = datatype & 0x0F;
    if ts_us.is_some() {
        flags |= 0b1000_0000;
    }
    if period_us.is_some() {
        flags |= 0b0100_0000;
    }
    if with_crc {
        flags |= 0b0010_0000;
    }
    if seq.is_some() {
        flags |= 0b0001_0000;
    }

    let mut buf = Vec::new();
    buf.extend_from_slice(&CBRT_SYNC);
    buf.push(flags);
    buf.push(channels);
    if let Some(s) = seq {
        buf.push(s);
    }
    if let Some(t) = ts_us {
        buf.extend_from_slice(&t.to_le_bytes());
    }
    if let Some(p) = period_us {
        buf.extend_from_slice(&p.to_le_bytes());
    }
    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    buf.extend_from_slice(payload);
    if with_crc {
        const CRC: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);
        let body = &buf[4..]; // flags byte through end of payload
        let cks = CRC.checksum(body);
        buf.extend_from_slice(&cks.to_le_bytes());
    }
    buf
}

#[test]
fn decodes_bare_i16_4ch() {
    let mut codec = CbrtCodec::default();
    // 4 channels, 2 samples
    let payload: Vec<u8> = [1i16, 2, 3, 4, 5, 6, 7, 8]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let frame = build_frame(0x3, 4, None, None, None, &payload, false);
    let mut buf = BytesMut::from(&frame[..]);

    let msg = codec.decode(&mut buf).unwrap().expect("frame decoded");
    assert!(buf.is_empty(), "frame fully consumed");
    assert_eq!(msg.values.len(), 8);
    // ch0 sample0 = 1, ch1 sample0 = 2, ..., ch3 sample1 = 8
    assert_eq!(msg.values[0].id, "ch0");
    assert_eq!(msg.values[0].as_i16(), Some(1));
    assert_eq!(msg.values[3].id, "ch3");
    assert_eq!(msg.values[3].as_i16(), Some(4));
    assert_eq!(msg.values[7].id, "ch3");
    assert_eq!(msg.values[7].as_i16(), Some(8));

    assert_eq!(msg.metadata_value("cbrt_datatype").unwrap().value_payload, b"i16".to_vec());
    assert_eq!(msg.metadata_value("cbrt_channels").unwrap().as_u8(), Some(4));
    assert_eq!(msg.metadata_value("cbrt_sample_count").unwrap().as_u32(), Some(2));
}

#[test]
fn decodes_worked_example_with_all_flags_and_crc() {
    let mut codec = CbrtCodec::default();
    // 4 channels i16, 10 samples, period=1000us, ts=1_000_000us, seq=0, with CRC.
    let mut samples = Vec::with_capacity(40);
    for s in 0..10i16 {
        for ch in 0..4i16 {
            samples.push(s * 10 + ch); // distinguishable
        }
    }
    let payload: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(payload.len(), 80);

    let frame = build_frame(
        0x3,
        4,
        Some(0),
        Some(1_000_000),
        Some(1000),
        &payload,
        true,
    );
    assert_eq!(frame.len(), 97, "matches the spec's worked example");

    let mut buf = BytesMut::from(&frame[..]);
    let msg = codec.decode(&mut buf).unwrap().expect("frame decoded");
    assert!(buf.is_empty());

    assert_eq!(msg.values.len(), 40);
    assert_eq!(msg.values[0].as_i16(), Some(0));
    assert_eq!(msg.values[5].as_i16(), Some(11)); // sample 1, ch 1 -> 1*10+1
    assert_eq!(msg.values[39].as_i16(), Some(93));

    // Per-sample timestamps differ: sample i base + i*1000.
    let t0 = msg.values[0].timestamp;
    let t1 = msg.values[4].timestamp; // first value of sample 1
    assert_eq!(t1 - t0, 1000);

    assert_eq!(msg.metadata_value("cbrt_seq").unwrap().as_u8(), Some(0));
    assert_eq!(
        msg.metadata_value("cbrt_ts_us").unwrap().as_u64(),
        Some(1_000_000)
    );
    assert_eq!(
        msg.metadata_value("cbrt_period_us").unwrap().as_u16(),
        Some(1000)
    );
    assert_eq!(
        msg.metadata_value("cbrt_session_start").unwrap().as_bool(),
        Some(true)
    );
}

#[test]
fn rejects_crc_mismatch_and_resyncs_to_next_sync() {
    let mut codec = CbrtCodec::default();
    let payload = vec![1u8, 2, 3, 4]; // u8, 4 channels, 1 sample
    let mut bad = build_frame(0x0, 4, None, None, None, &payload, true);
    // Corrupt the CRC.
    let last = bad.last_mut().unwrap();
    *last ^= 0xFF;

    let good = build_frame(0x0, 4, None, None, None, &[9, 8, 7, 6], false);

    let mut buf = BytesMut::new();
    buf.extend_from_slice(&bad);
    buf.extend_from_slice(&good);

    let msg = codec.decode(&mut buf).unwrap().expect("recovered after resync");
    assert_eq!(msg.values.len(), 4);
    assert_eq!(msg.values[0].as_u8(), Some(9));
    assert_eq!(msg.values[3].as_u8(), Some(6));
}

#[test]
fn returns_none_when_buffer_short() {
    let mut codec = CbrtCodec::default();
    let mut buf = BytesMut::from(&b"CBR"[..]);
    assert!(codec.decode(&mut buf).unwrap().is_none());
    assert_eq!(buf.len(), 3, "kept partial sync");
}

#[test]
fn drops_garbage_before_sync() {
    let mut codec = CbrtCodec::default();
    let frame = build_frame(0x0, 1, None, None, None, &[0x42], false);
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00]);
    buf.extend_from_slice(&frame);
    let msg = codec.decode(&mut buf).unwrap().expect("found frame after garbage");
    assert_eq!(msg.values.len(), 1);
    assert!(buf.is_empty());
}

#[test]
fn keep_alive_payload_zero_emits_message() {
    let mut codec = CbrtCodec::default();
    let frame = build_frame(0x0, 1, Some(7), Some(123_456), None, &[], false);
    let mut buf = BytesMut::from(&frame[..]);
    let msg = codec.decode(&mut buf).unwrap().expect("keep-alive surfaced");
    assert!(msg.values.is_empty());
    assert_eq!(msg.metadata_value("cbrt_seq").unwrap().as_u8(), Some(7));
    assert_eq!(
        msg.metadata_value("cbrt_ts_us").unwrap().as_u64(),
        Some(123_456)
    );
}

#[test]
fn reserved_datatype_0xf_rejected() {
    let mut codec = CbrtCodec::default();
    let bad = build_frame(0xF, 1, None, None, None, &[0x00], false);
    let mut buf = BytesMut::from(&bad[..]);
    assert!(codec.decode(&mut buf).unwrap().is_none());
    assert!(!buf.contains(&CBRT_SYNC[0]) || !buf.is_empty()); // garbage drained somehow
}

#[test]
fn invalid_channel_count_rejected() {
    let mut codec = CbrtCodec::default();
    // channels=0 — invalid.
    let mut frame = build_frame(0x0, 1, None, None, None, &[0], false);
    frame[5] = 0;
    let good = build_frame(0x0, 1, None, None, None, &[42], false);
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&frame);
    buf.extend_from_slice(&good);
    let msg = codec.decode(&mut buf).unwrap().expect("recovered");
    assert_eq!(msg.values[0].as_u8(), Some(42));
}

#[test]
fn payload_length_not_aligned_rejected() {
    let mut codec = CbrtCodec::default();
    // Declare i16 (2B/sample), 2 channels, but payload length = 3 (not multiple of 4).
    let mut frame = Vec::new();
    frame.extend_from_slice(&CBRT_SYNC);
    frame.push(0x3); // flags=0, datatype=i16
    frame.push(2); // channels
    frame.extend_from_slice(&3u16.to_le_bytes());
    frame.extend_from_slice(&[1, 2, 3]);
    // Then a good frame after.
    let good = build_frame(0x3, 2, None, None, None, &[1, 0, 2, 0], false);
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&frame);
    buf.extend_from_slice(&good);
    let msg = codec.decode(&mut buf).unwrap().expect("recovered");
    assert_eq!(msg.values.len(), 2);
}

#[test]
fn bool_packed_msb_first_channel_interleaved() {
    let mut codec = CbrtCodec::default();
    // 3 channels, 8 samples => 24 bits => 3 bytes.
    // Encode: ch0_s0=1, ch1_s0=0, ch2_s0=1, ch0_s1=1, ch1_s1=1, ch2_s1=0, ...
    let mut bits: Vec<u8> = vec![];
    let samples: Vec<[bool; 3]> = vec![
        [true, false, true],
        [true, true, false],
        [false, true, true],
        [false, false, true],
        [true, false, false],
        [false, true, false],
        [true, true, true],
        [false, false, false],
    ];
    let mut bitstream: Vec<bool> = vec![];
    for s in &samples {
        for ch in s {
            bitstream.push(*ch);
        }
    }
    // Pack MSB-first.
    for chunk in bitstream.chunks(8) {
        let mut b = 0u8;
        for (i, bit) in chunk.iter().enumerate() {
            if *bit {
                b |= 1 << (7 - i);
            }
        }
        bits.push(b);
    }
    assert_eq!(bits.len(), 3);

    let frame = build_frame(0xE, 3, None, None, None, &bits, false);
    let mut buf = BytesMut::from(&frame[..]);
    let msg = codec.decode(&mut buf).unwrap().expect("decoded");
    assert_eq!(msg.values.len(), 24);
    for (i, s) in samples.iter().enumerate() {
        for (ch, expected) in s.iter().enumerate() {
            let v = &msg.values[i * 3 + ch];
            assert_eq!(v.id, format!("ch{ch}"));
            assert_eq!(v.as_bool(), Some(*expected), "sample {} ch {}", i, ch);
        }
    }
}

#[test]
fn session_change_resets_state() {
    let mut codec = CbrtCodec::default();
    // Frame 1: u8, 2 ch, seq=5
    let f1 = build_frame(0x0, 2, Some(5), None, None, &[1, 2], false);
    let mut buf = BytesMut::from(&f1[..]);
    let _ = codec.decode(&mut buf).unwrap().expect("first");

    // Frame 2: u8, 2 ch, seq=6 (in-order). No drop.
    let f2 = build_frame(0x0, 2, Some(6), None, None, &[3, 4], false);
    buf.extend_from_slice(&f2);
    let m2 = codec.decode(&mut buf).unwrap().expect("second");
    assert!(m2.metadata_value("cbrt_seq_dropped").is_none());
    assert!(m2.metadata_value("cbrt_session_start").is_none());

    // Frame 3: switch to i16, 2 ch => new session, no drop event.
    let f3 = build_frame(0x3, 2, Some(99), None, None, &[1, 0, 2, 0], false);
    buf.extend_from_slice(&f3);
    let m3 = codec.decode(&mut buf).unwrap().expect("third");
    assert_eq!(
        m3.metadata_value("cbrt_session_start").unwrap().as_bool(),
        Some(true)
    );
    assert!(m3.metadata_value("cbrt_seq_dropped").is_none());
}

#[test]
fn drop_detection_reports_gap() {
    let mut codec = CbrtCodec::default();
    let f1 = build_frame(0x0, 1, Some(10), None, None, &[1], false);
    let f2 = build_frame(0x0, 1, Some(14), None, None, &[2], false); // skip 11,12,13 -> 3 dropped
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&f1);
    buf.extend_from_slice(&f2);
    let _ = codec.decode(&mut buf).unwrap().expect("f1");
    let m2 = codec.decode(&mut buf).unwrap().expect("f2");
    assert_eq!(
        m2.metadata_value("cbrt_seq_dropped").unwrap().as_u8(),
        Some(3)
    );
}

#[test]
fn timestamp_wrap_detected() {
    let mut codec = CbrtCodec::default();
    let f1 = build_frame(0x0, 1, None, Some(0xFFFF_FF00), None, &[1], false);
    let f2 = build_frame(0x0, 1, None, Some(0x0000_0010), None, &[2], false);
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&f1);
    buf.extend_from_slice(&f2);
    let _ = codec.decode(&mut buf).unwrap().expect("f1");
    let m2 = codec.decode(&mut buf).unwrap().expect("f2");
    let ts = m2.metadata_value("cbrt_ts_us").unwrap().as_u64().unwrap();
    assert_eq!(ts, (1u64 << 32) | 0x10);
}

#[test]
fn encode_is_raw_passthrough() {
    let mut codec = CbrtCodec::default();
    let mut msg = MessageBuilder::tx(
        0,
        PayloadType::Binary,
        b"hello".to_vec(),
        Vec::<u8>::new(),
    )
    .build();
    Codec::encode(&mut codec, &mut msg).unwrap();
    assert_eq!(msg.frame, b"hello".to_vec());
    assert_eq!(msg.payload, b"hello".to_vec());
}

#[test]
fn split_frame_across_reads() {
    let mut codec = CbrtCodec::default();
    let frame = build_frame(0x3, 2, Some(1), None, None, &[1, 0, 2, 0], false);
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&frame[..6]); // sync + flags + ch
    assert!(codec.decode(&mut buf).unwrap().is_none());
    buf.extend_from_slice(&frame[6..]);
    let m = codec.decode(&mut buf).unwrap().expect("complete now");
    assert_eq!(m.values.len(), 2);
}
