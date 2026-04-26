use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A minimal JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// A minimal JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

fn handle_request(request: &JsonRpcRequest) -> JsonRpcResponse {
    let id = request.id.clone().unwrap_or(Value::Null);

    match request.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "mock-mcp-echo",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        ),
        "tools/list" => JsonRpcResponse::success(
            id,
            serde_json::json!({
                "tools": [
                    {
                        "name": "echo",
                        "description": "Echoes back the input message.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "message": {
                                    "type": "string",
                                    "description": "The message to echo back."
                                }
                            },
                            "required": ["message"]
                        }
                    }
                ]
            }),
        ),
        "tools/call" => handle_tool_call(id, &request.params),
        "ping" => JsonRpcResponse::success(id, serde_json::json!({})),
        _ => JsonRpcResponse::error(id, -32601, format!("method not found: {}", request.method)),
    }
}

fn handle_tool_call(id: Value, params: &Value) -> JsonRpcResponse {
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");

    match tool_name {
        "echo" => {
            let message = params
                .get("arguments")
                .and_then(|a| a.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("");

            JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": message
                        }
                    ]
                }),
            )
        }
        _ => JsonRpcResponse::error(id, -32602, format!("unknown tool: {tool_name}")),
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(err) => {
                let response =
                    JsonRpcResponse::error(Value::Null, -32700, format!("parse error: {err}"));
                let out = serde_json::to_string(&response)?;
                writeln!(stdout, "{out}")?;
                stdout.flush()?;
                continue;
            }
        };

        // Notifications (no id) do not get responses per JSON-RPC spec.
        if request.id.is_none() {
            continue;
        }

        let response = handle_request(&request);
        let out = serde_json::to_string(&response)?;
        writeln!(stdout, "{out}")?;
        stdout.flush()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            id: Some(Value::Number(1.into())),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn initialize_returns_server_info() {
        let req = make_request("initialize", serde_json::json!({}));
        let resp = handle_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(
            result["serverInfo"]["name"].as_str().unwrap(),
            "mock-mcp-echo"
        );
    }

    #[test]
    fn tools_list_returns_echo_tool() {
        let req = make_request("tools/list", serde_json::json!({}));
        let resp = handle_request(&req);
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"].as_str().unwrap(), "echo");
    }

    #[test]
    fn echo_tool_returns_input_message() {
        let req = make_request(
            "tools/call",
            serde_json::json!({
                "name": "echo",
                "arguments": {"message": "hello world"}
            }),
        );
        let resp = handle_request(&req);
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn unknown_method_returns_error() {
        let req = make_request("nonexistent", serde_json::json!({}));
        let resp = handle_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn unknown_tool_returns_error() {
        let req = make_request(
            "tools/call",
            serde_json::json!({"name": "nonexistent", "arguments": {}}),
        );
        let resp = handle_request(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    #[test]
    fn ping_returns_empty_result() {
        let req = make_request("ping", serde_json::json!({}));
        let resp = handle_request(&req);
        let result = resp.result.unwrap();
        assert_eq!(result, serde_json::json!({}));
    }
}
