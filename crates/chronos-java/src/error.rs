#[derive(Debug, thiserror::Error)]
pub enum JavaError {
    #[error("Java not found in PATH (need java + javac)")]
    JavaNotFound,

    #[error("Failed to spawn JVM: {0}")]
    SpawnFailed(String),

    #[error("JDWP handshake failed: {0}")]
    JdwpHandshake(String),

    #[error("JDWP protocol error: {0}")]
    JdwpProtocol(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_error_display() {
        let err = JavaError::JavaNotFound;
        assert_eq!(err.to_string(), "Java not found in PATH (need java + javac)");

        let err = JavaError::SpawnFailed("No such file".to_string());
        assert_eq!(err.to_string(), "Failed to spawn JVM: No such file");

        let err = JavaError::JdwpHandshake("timeout".to_string());
        assert_eq!(err.to_string(), "JDWP handshake failed: timeout");

        let err = JavaError::JdwpProtocol("invalid packet".to_string());
        assert_eq!(err.to_string(), "JDWP protocol error: invalid packet");
    }
}
