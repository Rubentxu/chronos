#[derive(Debug, thiserror::Error)]
pub enum GoError {
    #[error("dlv (Delve) not found in PATH")]
    DelveNotFound,

    #[error("Go not found in PATH")]
    GoNotFound,

    #[error("Failed to spawn Delve: {0}")]
    SpawnFailed(String),

    #[error("Delve RPC error: {0}")]
    RpcError(String),

    #[error("JSON parse error: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_error_display() {
        let err = GoError::DelveNotFound;
        assert_eq!(err.to_string(), "dlv (Delve) not found in PATH");

        let err = GoError::GoNotFound;
        assert_eq!(err.to_string(), "Go not found in PATH");

        let err = GoError::SpawnFailed("No such file".to_string());
        assert_eq!(err.to_string(), "Failed to spawn Delve: No such file");

        let err = GoError::RpcError("connection refused".to_string());
        assert_eq!(err.to_string(), "Delve RPC error: connection refused");
    }
}
