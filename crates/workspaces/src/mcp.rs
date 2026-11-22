use std::io::{self, BufRead, Read, Write};
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{WorkspaceStore, operations};

const VERSION: &str = "2025-11-25";
const MAX_MESSAGE_BYTES: u64 = 128 * 1024;

#[derive(Default)]
enum Session {
    #[default]
    New,
    Initializing,
    Ready,
}

#[derive(Deserialize)]
struct ToolCall {
    name: String,
    #[serde(default)]
    arguments: serde_json::Map<String, Value>,
}

pub fn serve_mcp(
    store: &mut WorkspaceStore,
    database: &Path,
    mut input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()> {
    let mut session = Session::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        if input
            .by_ref()
            .take(MAX_MESSAGE_BYTES + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() as u64 > MAX_MESSAGE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP message exceeds 128 KiB",
            ));
        }
        let response = match serde_json::from_slice(&line) {
            Ok(message) => dispatch(store, database, &mut session, message),
            Err(_) => Some(rpc_error(Value::Null, -32700, "Parse error")),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response).map_err(io::Error::other)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

fn dispatch(
    store: &mut WorkspaceStore,
    database: &Path,
    session: &mut Session,
    message: Value,
) -> Option<Value> {
    let request_id = message.get("id");
    let id = request_id.cloned().unwrap_or(Value::Null);
    let method = message["method"].as_str();
    if message["jsonrpc"] != "2.0"
        || method.is_none()
        || request_id.is_some_and(|id| !(id.is_string() || id.is_i64() || id.is_u64()))
    {
        return Some(rpc_error(Value::Null, -32600, "Invalid request"));
    }
    let method = method?;
    if request_id.is_none() {
        if method == "notifications/initialized" && matches!(session, Session::Initializing) {
            *session = Session::Ready;
        }
        return None;
    }
    let result = match method {
        "ping" => json!({}),
        "initialize" if matches!(session, Session::New) => {
            if message["params"]["protocolVersion"].as_str().is_none()
                || !message["params"]["capabilities"].is_object()
                || !message["params"]["clientInfo"].is_object()
            {
                return Some(rpc_error(id, -32602, "Invalid initialization parameters"));
            }
            *session = Session::Initializing;
            json!({
                "protocolVersion": VERSION,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "arrumo", "version": env!("CARGO_PKG_VERSION")}
            })
        }
        "initialize" => return Some(rpc_error(id, -32600, "MCP session already initialized")),
        _ if !matches!(session, Session::Ready) => {
            return Some(rpc_error(id, -32002, "MCP session is not initialized"));
        }
        "tools/list" => json!({"tools": operations::definitions()}),
        "tools/call" => {
            let Some(params) = message.get("params") else {
                return Some(rpc_error(id, -32602, "Missing tool call parameters"));
            };
            let Ok(call) = ToolCall::deserialize(params) else {
                return Some(rpc_error(id, -32602, "Invalid tool call parameters"));
            };
            match call_tool(store, database, call) {
                Some(result) => result,
                None => return Some(rpc_error(id, -32602, "Unknown tool")),
            }
        }
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn call_tool(store: &mut WorkspaceStore, database: &Path, call: ToolCall) -> Option<Value> {
    Some(
        match operations::invoke(store, database, &call.name, call.arguments)? {
            Ok(data) => json!({
                "content": [{"type": "text", "text": data.to_string()}],
                "structuredContent": data
            }),
            Err(error) => tool_error(&error),
        },
    )
}

fn tool_error(message: &str) -> Value {
    json!({"isError": true, "content": [{"type": "text", "text": message}]})
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiates_stdio_session_and_only_replies_to_requests() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        store.create("Studio").unwrap();
        let requests = r#"
{"jsonrpc":"2.0","id":1,"method":"tools/list"}
{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"fixture","version":"1"},"capabilities":{}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":"list","method":"tools/list"}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_workspaces"}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"list_plugin_actions","arguments":{"unexpected":true}}}
"#;
        let mut output = Vec::new();
        serve_mcp(
            &mut store,
            Path::new(":memory:"),
            requests.trim_start().as_bytes(),
            &mut output,
        )
        .unwrap();
        let responses: Vec<Value> = std::str::from_utf8(&output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 5);
        assert_eq!(responses[0]["error"]["code"], -32002);
        assert_eq!(responses[1]["result"]["protocolVersion"], VERSION);
        assert_eq!(responses[2]["id"], "list");
        assert_eq!(
            responses[2]["result"]["tools"].as_array().unwrap().len(),
            175
        );
        assert_eq!(
            responses[3]["result"]["structuredContent"]["workspaces"][0]["name"],
            "Studio"
        );
        assert_eq!(responses[4]["result"]["isError"], true);
    }

    #[test]
    fn generation_failures_tell_the_agent_why() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let call = |store: &mut WorkspaceStore, name: &str, arguments: Value| {
            call_tool(
                store,
                &database,
                ToolCall {
                    name: name.into(),
                    arguments: arguments.as_object().unwrap().clone(),
                },
            )
            .unwrap()
        };
        let model_alone = call(
            &mut store,
            "generate_library_video",
            json!({"workspaceId": workspace, "prompt": "Waves", "aspectRatio": "16:9", "model": "seedance-2.0"}),
        );
        assert_eq!(model_alone["isError"], true);
        assert!(
            model_alone["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Name the provider whose model you want")
        );
        let foreign = call(
            &mut store,
            "generate_library_images",
            json!({"workspaceId": workspace, "prompt": "Desk", "aspectRatio": "16:9", "count": 1, "references": [999]}),
        );
        assert!(
            foreign["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("not in this workspace")
        );
    }

    #[test]
    fn imports_context_suggestions_like_the_desktop() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("studio.sqlite");
        let mut store = WorkspaceStore::open(&database).unwrap();
        let workspace = store.create("Studio").unwrap().id;
        let call = |store: &mut WorkspaceStore, arguments: Value| {
            call_tool(
                store,
                &database,
                ToolCall {
                    name: "import_workspace_context".into(),
                    arguments: arguments.as_object().unwrap().clone(),
                },
            )
            .unwrap()
        };
        let result = call(
            &mut store,
            json!({"workspaceId": workspace, "colors": ["#ffcc00"],
                "profiles": [{"name": "Speedruns", "description": "Fast solves", "guidance": "", "color": null}]}),
        );
        assert_eq!(result["structuredContent"]["colors"], 1);
        assert_eq!(result["structuredContent"]["profiles"], 1);
        assert_eq!(store.brand_colors(workspace).unwrap(), vec!["#FFCC00"]);
        let rejected = call(&mut store, json!({"workspaceId": workspace, "extra": true}));
        assert_eq!(rejected["isError"], true);
    }

    #[test]
    fn rejects_overlong_stdio_messages_before_parsing() {
        let mut store = WorkspaceStore::open(":memory:").unwrap();
        let input = vec![b' '; MAX_MESSAGE_BYTES as usize + 1];
        let mut output = Vec::new();
        assert!(
            serve_mcp(
                &mut store,
                Path::new(":memory:"),
                input.as_slice(),
                &mut output
            )
            .is_err()
        );
        assert!(output.is_empty());
    }
}
