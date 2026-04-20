//! Capture pipeline — orchestrates event flow from adapter to storage.

use chronos_domain::{TraceError, TraceEvent};
use tokio::sync::mpsc;

/// Channel capacity for the event pipeline.
const CHANNEL_CAPACITY: usize = 10_000;

/// The capture pipeline coordinates event flow:
/// 1. Adapter produces events → mpsc channel
/// 2. Pipeline consumers receive events → write to trace file + build indices
pub struct CapturePipeline {
    /// Sender half of the event channel.
    event_tx: mpsc::Sender<TraceEvent>,
    /// Receiver half of the event channel.
    event_rx: Option<mpsc::Receiver<TraceEvent>>,
}

impl CapturePipeline {
    /// Create a new capture pipeline with a bounded channel.
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        Self {
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Create with a custom channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (event_tx, event_rx) = mpsc::channel(capacity);
        Self {
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Get a sender for producing events.
    pub fn sender(&self) -> mpsc::Sender<TraceEvent> {
        self.event_tx.clone()
    }

    /// Take the receiver (can only be called once).
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<TraceEvent>> {
        self.event_rx.take()
    }

    /// Send an event through the pipeline.
    /// Applies backpressure when the channel is full.
    pub async fn send(&self, event: TraceEvent) -> Result<(), TraceError> {
        self.event_tx
            .send(event)
            .await
            .map_err(|e| TraceError::InternalError(format!("Channel send failed: {}", e)))
    }

    /// Try to send an event without waiting.
    /// Returns Ok(false) if the channel is full (backpressure).
    pub fn try_send(&self, event: TraceEvent) -> Result<bool, TraceError> {
        match self.event_tx.try_send(event) {
            Ok(()) => Ok(true),
            Err(mpsc::error::TrySendError::Full(_)) => Ok(false),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(TraceError::InternalError("Channel closed".to_string()))
            }
        }
    }
}

impl Default for CapturePipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::EventType;

    #[tokio::test]
    async fn test_pipeline_send_receive() {
        let mut pipeline = CapturePipeline::with_capacity(100);
        let mut rx = pipeline.take_receiver().unwrap();

        let event = TraceEvent::function_entry(1, 100, 1, "main", 0x1000);
        pipeline.send(event.clone()).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_id, 1);
    }

    #[tokio::test]
    async fn test_pipeline_backpressure() {
        let pipeline = CapturePipeline::with_capacity(2);

        // Fill the channel
        assert!(pipeline.try_send(TraceEvent::function_entry(1, 100, 1, "a", 0x1)).unwrap());
        assert!(pipeline.try_send(TraceEvent::function_entry(2, 200, 1, "b", 0x2)).unwrap());

        // Channel is full, backpressure
        assert!(!pipeline.try_send(TraceEvent::function_entry(3, 300, 1, "c", 0x3)).unwrap());
    }

    #[tokio::test]
    async fn test_pipeline_default() {
        let p = CapturePipeline::default();
        assert!(p.sender().capacity() > 0);
    }
}
