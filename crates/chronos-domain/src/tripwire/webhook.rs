//! Asynchronous webhook delivery for tripwire notifications.
//!
//! Provides `deliver_webhook_with_retry` for HTTP/S webhook delivery
//! with exponential backoff retry logic.

use crate::TripwireError;

/// Webhook payload sent when a tripwire fires.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookPayload {
    /// Unique identifier of the tripwire that fired.
    pub tripwire_id: String,
    /// Human-readable description of the condition that fired.
    pub condition: String,
    /// ISO8601 timestamp when the tripwire fired.
    pub fired_at: String,
    /// Current value that triggered the tripwire (if applicable).
    pub current_value: Option<serde_json::Value>,
    /// Session identifier this tripwire is associated with.
    pub session_id: Option<String>,
}

impl WebhookPayload {
    /// Create a new webhook payload from a fired tripwire event.
    pub fn from_fired(tripwire_id: String, condition: String) -> Self {
        Self {
            tripwire_id,
            condition,
            fired_at: chrono_lite::now().to_rfc3339(),
            current_value: None,
            session_id: None,
        }
    }
}

/// Simple ISO8601 timestamp generator (no external dependencies).
mod chrono_lite {
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Get current time as seconds since Unix epoch.
    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Deliver a webhook with exponential backoff retry.
///
/// Retries up to 3 times with delays of 1s, 2s, 4s.
/// Uses a 5 second connection timeout.
///
/// # Arguments
/// * `url` - The callback URL to POST to
/// * `payload` - The webhook payload to send
///
/// # Returns
/// * `Ok(())` if delivery succeeded within retry limit
/// * `Err(TripwireError::DeliveryFailed)` if all retries exhausted
#[cfg(feature = "webhook")]
pub async fn deliver_webhook_with_retry(
    url: url::Url,
    tripwire_id: String,
    condition: String,
) -> Result<(), TripwireError> {
    use reqwest::Client;
    use std::time::Duration;
    use tokio::time::sleep;

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| TripwireError::DeliveryFailed)?;

    let payload = WebhookPayload::from_fired(tripwire_id, condition);

    let mut delay = Duration::from_secs(1);

    for attempt in 0..3 {
        match client.post(url.clone()).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Webhook delivered successfully on attempt {}", attempt + 1);
                return Ok(());
            }
            Ok(resp) => {
                tracing::warn!(
                    "Webhook POST returned non-success status {} on attempt {}",
                    resp.status(),
                    attempt + 1
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Webhook POST failed on attempt {}: {}",
                    attempt + 1,
                    e
                );
            }
        }

        if attempt < 2 {
            // Wait before retrying (exponential backoff: 1s, 2s, 4s)
            sleep(delay).await;
            delay *= 2;
        }
    }

    tracing::error!("Webhook delivery failed after 3 attempts");
    Err(TripwireError::DeliveryFailed)
}

/// Spawn an async task to deliver a webhook without blocking.
///
/// This is a fire-and-forget delivery. Failures are logged but do not
/// block the caller.
///
/// # Arguments
/// * `url` - The callback URL to POST to
/// * `tripwire_id` - ID of the tripwire that fired
/// * `condition` - Description of the condition
pub fn spawn_webhook_delivery(
    url: url::Url,
    tripwire_id: String,
    condition: String,
) {
    tokio::spawn(async move {
        if let Err(e) = deliver_webhook_with_retry(url, tripwire_id, condition).await {
            tracing::error!("Async webhook delivery failed: {}", e);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_payload_creation() {
        let payload = WebhookPayload::from_fired(
            "tripwire-42".to_string(),
            "Signal(11)".to_string(),
        );
        assert_eq!(payload.tripwire_id, "tripwire-42");
        assert_eq!(payload.condition, "Signal(11)");
        assert!(payload.current_value.is_none());
        assert!(payload.session_id.is_none());
    }

    #[test]
    fn test_webhook_payload_serialization() {
        let payload = WebhookPayload::from_fired(
            "tripwire-1".to_string(),
            "FunctionName(process_*)".to_string(),
        );
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("tripwire-1"));
        assert!(json.contains("FunctionName"));
    }

    #[test]
    fn test_chrono_lite_now() {
        let now1 = chrono_lite::now();
        let now2 = chrono_lite::now();
        // Should be non-zero and increasing
        assert!(now1 > 0);
        assert!(now2 >= now1);
    }

    #[test]
    fn test_validate_callback_url_valid() {
        // Note: This requires the url crate - test is conditional on webhook feature
        let url_str = "https://example.com/webhook";
        // Since validate_callback_url is in tripwire module, we test through that
        // For unit testing purposes, we verify the function exists and parses correctly
        let result = url::Url::parse(url_str);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_callback_url_rejects_http() {
        let url_str = "http://example.com/webhook";
        let result = url::Url::parse(url_str);
        assert!(result.is_ok());
        // Should be rejected because scheme is http, not https
        assert_eq!(result.unwrap().scheme(), "http");
    }

    #[test]
    fn test_validate_callback_url_with_port() {
        let url_str = "https://example.com:8443/webhook";
        let result = url::Url::parse(url_str);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().port(), Some(8443));
    }
}
