//! High-level CDP Debugger domain wrapper.

use crate::cdp_client::{CdpClient, Property, RemoteObject};
use crate::error::JsAdapterError;
use std::sync::Arc;

/// High-level debugger interface for JavaScript debugging via CDP.
#[derive(Clone)]
pub struct JsDebugger {
    client: Arc<CdpClient>,
}

impl JsDebugger {
    /// Create a new debugger wrapping a CDP client
    pub fn new(client: Arc<CdpClient>) -> Self {
        Self { client }
    }

    /// Enable the debugger domain
    pub async fn enable(&self) -> Result<(), JsAdapterError> {
        self.client.debugger_enable().await
    }

    /// Enable the runtime domain
    pub async fn enable_runtime(&self) -> Result<(), JsAdapterError> {
        self.client.runtime_enable().await
    }

    /// Resume execution
    pub async fn resume(&self) -> Result<(), JsAdapterError> {
        self.client.debugger_resume().await
    }

    /// Step over the current line
    pub async fn step_over(&self) -> Result<(), JsAdapterError> {
        self.client.debugger_step_over().await
    }

    /// Step into a function call
    pub async fn step_into(&self) -> Result<(), JsAdapterError> {
        self.client.debugger_step_into().await
    }

    /// Step out of the current function
    pub async fn step_out(&self) -> Result<(), JsAdapterError> {
        self.client.debugger_step_out().await
    }

    /// Get properties of a remote object (for locals, globals, etc.)
    pub async fn get_properties(&self, object_id: &str) -> Result<Vec<Property>, JsAdapterError> {
        self.client.runtime_get_properties(object_id).await
    }

    /// Subscribe to debugger events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<crate::cdp_client::CdpEvent> {
        self.client.subscribe()
    }
}

/// Convert CDP RemoteObject to VariableInfo
pub fn remote_object_to_variable_info(
    name: &str,
    remote: &RemoteObject,
) -> chronos_domain::VariableInfo {
    let value = remote
        .value
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default())
        .unwrap_or_else(|| remote.description.clone().unwrap_or_default());

    let type_name = remote.subtype.clone().unwrap_or_else(|| remote.type_.clone());

    let address = remote
        .object_id
        .as_ref()
        .and_then(|id| parse_hex_address(id))
        .unwrap_or(0);

    chronos_domain::VariableInfo::new(
        name,
        &value,
        &type_name,
        address,
        chronos_domain::VariableScope::Local,
    )
}

/// Parse a hex address from CDP object ID
fn parse_hex_address(s: &str) -> Option<u64> {
    // CDP object IDs are typically hex strings
    let cleaned = s.trim_start_matches("0x");
    u64::from_str_radix(cleaned, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_address() {
        assert_eq!(parse_hex_address("0x1234"), Some(0x1234));
        assert_eq!(parse_hex_address("deadbeef"), Some(0xdeadbeef));
        assert_eq!(parse_hex_address("invalid"), None);
    }
}
