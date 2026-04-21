//! Minimal Delve JSON-RPC 2.0 client over TCP.

use crate::error::GoError;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// A Delve DAP client for JSON-RPC 2.0 communication.
pub struct DelveClient {
    read_half: BufReader<tokio::net::tcp::OwnedReadHalf>,
    write_half: tokio::net::tcp::OwnedWriteHalf,
    next_id: u64,
}

impl DelveClient {
    /// Connect to a Delve DAP server at the given port.
    pub async fn connect(port: u16) -> Result<Self, GoError> {
        let addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&addr).await?;
        let (read_half, write_half) = stream.into_split();
        let read_buf = BufReader::new(read_half);

        Ok(Self {
            read_half: read_buf,
            write_half,
            next_id: 1,
        })
    }

    /// Send a JSON-RPC 2.0 request and parse the response.
    async fn call(&mut self, method: &str, params: Value) -> Result<Value, GoError> {
        let id = self.next_id;
        self.next_id += 1;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id
        });

        // Send request
        let request_str = serde_json::to_string(&request)? + "\n";
        self.write_half.write_all(request_str.as_bytes()).await?;

        // Read response
        let mut line = String::new();
        self.read_half.read_line(&mut line).await?;

        let response: Value = serde_json::from_str(&line)?;

        // Check for error
        if let Some(error) = response.get("error") {
            return Err(GoError::RpcError(error.to_string()));
        }

        // Extract result
        response
            .get("result")
            .cloned()
            .ok_or_else(|| GoError::RpcError("No result in response".to_string()))
    }

    /// RPCServer.ProcessPid — get the process ID.
    pub async fn get_pid(&mut self) -> Result<u64, GoError> {
        let result = self
            .call("RPCServer.ProcessPid", serde_json::json!({}))
            .await?;
        Ok(result.as_i64().unwrap_or(0) as u64)
    }

    /// RPCServer.State — get current debugger state.
    pub async fn get_state(&mut self) -> Result<DelveState, GoError> {
        let result = self.call("RPCServer.State", serde_json::json!({})).await?;
        let state: DelveState = serde_json::from_value(result)?;
        Ok(state)
    }

    /// RPCServer.Stacktrace for a goroutine.
    pub async fn stacktrace(
        &mut self,
        goroutine_id: i64,
        depth: i32,
    ) -> Result<Vec<StackFrame>, GoError> {
        let result = self
            .call(
                "RPCServer.Stacktrace",
                serde_json::json!({
                    "id": goroutine_id,
                    "depth": depth,
                    "flags": 0,
                    "regs": null,
                    "locals": false,
                    "args": false,
                    "maxStructFields": -1
                }),
            )
            .await?;
        let frames: Vec<StackFrame> = serde_json::from_value(result)?;
        Ok(frames)
    }

    /// RPCServer.ListGoroutines — list all goroutines.
    pub async fn list_goroutines(&mut self) -> Result<Vec<GoroutineInfo>, GoError> {
        let result = self
            .call("RPCServer.ListGoroutines", serde_json::json!({}))
            .await?;
        #[derive(Deserialize)]
        struct GoroutinesReply {
            #[serde(rename = "Goroutines")]
            goroutines: Vec<GoroutineInfo>,
        }
        let reply: GoroutinesReply = serde_json::from_value(result)?;
        Ok(reply.goroutines)
    }

    /// RPCServer.Command — execute a debug command (continue, next, step, stepout).
    pub async fn command(&mut self, name: &str) -> Result<DelveState, GoError> {
        let result = self
            .call(
                "RPCServer.Command",
                serde_json::json!({
                    "name": name,
                    "threadID": 0,
                    "goroutineID": -1
                }),
            )
            .await?;
        let state: DelveState = serde_json::from_value(result)?;
        Ok(state)
    }
}

// Delve response types

#[derive(Debug, Deserialize)]
pub struct StackFrame {
    pub function: Option<FunctionInfo>,
    pub file: String,
    pub line: i64,
    #[serde(rename = "Locals")]
    pub locals: Option<Vec<DelveVar>>,
}

#[derive(Debug, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DelveVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct GoroutineInfo {
    pub id: i64,
    #[serde(rename = "currentLoc")]
    pub currentLoc: StackFrame,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct DelveState {
    pub exited: Option<bool>,
    #[serde(rename = "currentThread")]
    pub currentThread: Option<ThreadInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct ThreadInfo {
    #[serde(rename = "goroutineID")]
    pub goroutineID: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delve_rpc_request_format() {
        // Verify JSON-RPC 2.0 request format
        let method = "RPCServer.ProcessPid";
        let id = 42u64;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": {},
            "id": id
        });

        let request_str = serde_json::to_string(&request).unwrap();
        assert!(request_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(request_str.contains("\"method\":\"RPCServer.ProcessPid\""));
        assert!(request_str.contains("\"id\":42"));
    }

    #[test]
    fn test_delve_rpc_response_parse() {
        // Sample stacktrace response from Delve
        // Note: In real Delve responses, function info is nested inside userCurrentLoc
        let response_json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "id": 0,
                    "userCurrentLoc": {
                        "pc": 12345678,
                        "file": "/path/to/main.go",
                        "line": 10,
                        "function": {"name": "main.main"}
                    },
                    "callLoc": {
                        "pc": 0,
                        "file": "",
                        "line": 0,
                        "function": null
                    },
                    "func": {
                        "name": "main.main",
                        "type": 2,
                        "value": 0,
                        "goType": 0
                    },
                    "file": "/path/to/main.go",
                    "line": 10,
                    "Locals": [
                        {"name": "x", "value": "10", "type": "int", "flags": 0, "typename": "int", "realtypename": "int", "class": "VariableParm"}
                    ],
                    "Parent": null
                }
            ]
        }"#;

        let response: Value = serde_json::from_str(response_json).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);

        let result = response["result"].as_array().unwrap();
        assert_eq!(result.len(), 1);

        let frame = &result[0];
        // In Delve responses, file and line are at the frame level
        assert_eq!(frame["file"], "/path/to/main.go");
        assert_eq!(frame["line"], 10);
        // Function info is nested inside userCurrentLoc
        assert_eq!(frame["userCurrentLoc"]["function"]["name"], "main.main");

        let locals = frame["Locals"].as_array().unwrap();
        assert_eq!(locals[0]["name"], "x");
        assert_eq!(locals[0]["value"], "10");
    }

    #[test]
    fn test_delve_state_parse() {
        let state_json = r#"{
            "exited": false,
            "currentThread": {
                "id": 1,
                "name": "Main goroutine",
                "pc": 12345678,
                "file": "/path/to/main.go",
                "line": 10,
                "function": {"name": "main.main"},
                "goroutineID": 1
            }
        }"#;

        let state: DelveState = serde_json::from_str(state_json).unwrap();
        assert_eq!(state.exited, Some(false));
        assert!(state.currentThread.is_some());
        assert_eq!(state.currentThread.as_ref().unwrap().goroutineID, 1);
    }

    #[test]
    fn test_goroutine_info_parse() {
        let goroutine_json = r#"{
            "id": 1,
            "currentLoc": {
                "pc": 12345678,
                "file": "/path/to/main.go",
                "line": 10,
                "function": {"name": "main.main"}
            }
        }"#;

        let info: GoroutineInfo = serde_json::from_str(goroutine_json).unwrap();
        assert_eq!(info.id, 1);
        assert_eq!(info.currentLoc.file, "/path/to/main.go");
        assert_eq!(info.currentLoc.line, 10);
    }
}
