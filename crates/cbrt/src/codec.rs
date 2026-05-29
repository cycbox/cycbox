use crate::parser::{CBRT_SYNC, ParseOutcome, SessionState, parse_at};
use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use cycbox_sdk::prelude::*;

pub const CBRT_CODEC_ID: &str = "cbrt_codec";

#[derive(Debug, Clone, Default)]
pub struct CbrtCodec {
    state: SessionState,
}

#[async_trait]
impl Codec for CbrtCodec {
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        let arrival_us = Message::current_timestamp();

        loop {
            let buffer = src.as_ref();

            if buffer.len() < CBRT_SYNC.len() {
                return Ok(None);
            }

            // Find next sync from the front of the buffer.
            let sync_pos = buffer
                .windows(CBRT_SYNC.len())
                .position(|w| w == CBRT_SYNC);

            let sync_pos = match sync_pos {
                Some(i) => i,
                None => {
                    // Keep the last 3 bytes in case the sync is split across reads.
                    let keep = CBRT_SYNC.len() - 1;
                    if buffer.len() > keep {
                        src.advance(buffer.len() - keep);
                    }
                    return Ok(None);
                }
            };

            match parse_at(&mut self.state, buffer, sync_pos, arrival_us) {
                ParseOutcome::Complete { frame_end, message } => {
                    src.advance(frame_end);
                    return Ok(Some(message));
                }
                ParseOutcome::NeedMore => {
                    // Drop any leading garbage before the sync; wait for more bytes.
                    if sync_pos > 0 {
                        src.advance(sync_pos);
                    }
                    return Ok(None);
                }
                ParseOutcome::Reject => {
                    // §6.2: advance 1 byte past the start of the rejected sync, rescan.
                    src.advance(sync_pos + 1);
                    continue;
                }
            }
        }
    }

    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError> {
        // Raw passthrough: send payload as-is, no framing added.
        if item.frame.is_empty() {
            item.frame = item.payload.clone();
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.state.reset();
    }
}
