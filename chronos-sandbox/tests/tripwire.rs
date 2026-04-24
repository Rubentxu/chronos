//! Tripwire tests — verify tripwire creation and listing.

use chronos_sandbox::McpTestClient;
use chronos_sandbox::client::types::{TripwireConditionType, TripwireCreateParams};

/// Stub test — requires chronos-mcp binary to exist.
/// Run with: cargo test -p chronos-sandbox -- --ignored
#[tokio::test]
#[ignore]
async fn test_tripwire_create_and_list() {
    let mut client = McpTestClient::start().await.unwrap();
    let id = client
        .tripwire_create(TripwireCreateParams {
            condition: TripwireConditionType::FunctionName {
                pattern: "main".into(),
            },
            label: None,
        })
        .await
        .unwrap();
    let list = client.tripwire_list().await.unwrap();
    assert!(list.iter().any(|t| t.id == id));
}
