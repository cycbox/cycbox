use crate::delay::HighResDelay;
use crate::engine::EngineRef;
use cycbox_sdk::Message;
use log::{debug, warn};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// A message scheduled to be sent at a specific time
#[derive(Debug)]
struct ScheduledMessage {
    pub message: Message,
    pub deadline: Instant,
}

impl ScheduledMessage {
    fn from_message(message: Message) -> Self {
        let target = std::time::UNIX_EPOCH + Duration::from_micros(message.timestamp);
        let deadline = match target.duration_since(std::time::SystemTime::now()) {
            Ok(remaining) => Instant::now() + remaining,
            Err(_) => Instant::now(),
        };
        Self { message, deadline }
    }
}

// Implement ordering for priority queue (min-heap by deadline)
impl Ord for ScheduledMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.deadline.cmp(&self.deadline)
    }
}

impl PartialOrd for ScheduledMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for ScheduledMessage {}

impl PartialEq for ScheduledMessage {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

/// background task that processes delayed messages
/// messages are sent from the main task with a timestamp in the future,
/// and this task will wait until the timestamp and then send the message via engine.send_message()
pub fn start_delay_queue_task(
    engine: EngineRef,
    mut receiver: mpsc::Receiver<Message>,
    ctx: CancellationToken,
) -> JoinHandle<()> {
    crate::RUNTIME.spawn(async move {
        let mut queue: BinaryHeap<ScheduledMessage> = BinaryHeap::new();
        let mut delay = match HighResDelay::new() {
            Ok(d) => d,
            Err(e) => {
                warn!("DelayQueueTask: Failed to create HighResDelay: {}", e);
                return;
            }
        };

        loop {
            if let Some(scheduled) = queue.peek() {
                let now = Instant::now();
                let deadline = scheduled.deadline;

                if deadline <= now {
                    // Message is ready to send
                    if let Some(scheduled) = queue.pop() {
                        engine.send_message(scheduled.message).await;
                    }
                    continue; // Check for more ready messages
                }

                // Calculate time to wait
                let wait = deadline - now;

                tokio::select! {
                    result = delay.delay(wait) => {
                        if let Err(e) = result {
                            warn!("DelayQueueTask: Delay error: {}", e);
                        }
                        // Loop will re-check queue
                    }
                    msg = receiver.recv() => {
                        match msg {
                            Some(msg) => {
                                let _ = delay.disarm();
                                queue.push(ScheduledMessage::from_message(msg));
                            }
                            None => {
                                debug!("DelayQueueTask: Receiver channel closed, exiting");
                                return;
                            }
                        }
                    }
                    _ = ctx.cancelled() => {
                        debug!("DelayQueueTask: Cancelled, exiting");
                        return;
                    }
                }
            } else {
                // Queue is empty, wait for new messages
                debug!("DelayQueueTask: Queue empty, waiting for messages");
                tokio::select! {
                    msg = receiver.recv() => {
                        match msg {
                            Some(msg) => {
                                debug!("DelayQueueTask: Received new scheduled message");
                                queue.push(ScheduledMessage::from_message(msg));
                            }
                            None => {
                                debug!("DelayQueueTask: Receiver channel closed, exiting");
                                return;
                            }
                        }
                    }
                    _ = ctx.cancelled() => {
                        debug!("DelayQueueTask: Cancelled, exiting");
                        return;
                    }
                }
            }
        }
    })
}
