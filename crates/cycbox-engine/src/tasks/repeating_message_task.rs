use crate::delay::HighResDelay;
use crate::engine::EngineRef;
use cycbox_sdk::Message;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// background task that processes repeating messages
/// messages are sent from the main task with a batch_id and a list of (delay, message) pairs,
/// and this task will send the messages in sequence with the specified delays, and repeat
pub(crate) fn start_repeating_message_task(
    engine: EngineRef,
    messages_with_delays: Vec<(Duration, Message)>,
    ctx: CancellationToken,
) -> JoinHandle<()> {
    crate::RUNTIME.spawn(async move {
        let mut delay = match HighResDelay::new() {
            Ok(d) => d,
            Err(_) => return,
        };
        loop {
            // check for cancellation before each batch
            if ctx.is_cancelled() {
                break;
            }
            for (duration, message) in &messages_with_delays {
                let _ = delay.delay(*duration).await;
                if ctx.is_cancelled() {
                    break;
                }
                let mut new_message = message.clone();
                new_message.refresh_timestamp();
                engine.send_message(new_message).await;
            }
        }
    })
}
