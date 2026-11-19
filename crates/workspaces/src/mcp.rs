use std::io::{self, BufRead, Read, Write};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::WorkspaceStore;

mod generation;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ActionArguments {
    plugin_id: String,
    action_id: String,
    package_sha256: String,
    workspace_id: Option<i64>,
    input: Value,
}

pub fn serve_mcp(
    store: &WorkspaceStore,
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
            Ok(message) => dispatch(store, &mut session, message),
            Err(_) => Some(rpc_error(Value::Null, -32700, "Parse error")),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut output, &response).map_err(io::Error::other)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

fn dispatch(store: &WorkspaceStore, session: &mut Session, message: Value) -> Option<Value> {
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
        "tools/list" => tool_definitions(),
        "tools/call" => {
            let Some(params) = message.get("params") else {
                return Some(rpc_error(id, -32602, "Missing tool call parameters"));
            };
            let Ok(call) = ToolCall::deserialize(params) else {
                return Some(rpc_error(id, -32602, "Invalid tool call parameters"));
            };
            match call_tool(store, call) {
                Some(result) => result,
                None => return Some(rpc_error(id, -32602, "Unknown tool")),
            }
        }
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn tool_definitions() -> Value {
    let no_input = json!({"type": "object", "additionalProperties": false});
    let mut tools = vec![
        json!({
            "name": "list_workspaces",
            "description": "List local Arrumo workspaces and their IDs.",
            "inputSchema": no_input,
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "list_plugin_actions",
            "description": "List installed plugin actions, their scopes and current package digests.",
            "inputSchema": no_input,
            "annotations": {"readOnlyHint": true}
        }),
        json!({
            "name": "invoke_plugin_action",
            "description": "Invoke a declared plugin action in its app or workspace scope. Use the package digest returned by list_plugin_actions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pluginId": {"type": "string"},
                    "actionId": {"type": "string"},
                    "packageSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "workspaceId": {"type": "integer", "minimum": 1, "description": "Required for workspace actions. Omit for app actions."},
                    "input": {}
                },
                "required": ["pluginId", "actionId", "packageSha256", "input"],
                "additionalProperties": false
            }
        }),
    ];
    tools.extend(generation::definitions());
    json!({"tools": tools})
}

fn call_tool(store: &WorkspaceStore, call: ToolCall) -> Option<Value> {
    let result = match call.name.as_str() {
        "list_workspaces" if call.arguments.is_empty() => store
            .list()
            .map(|workspaces| json!({"workspaces": workspaces}))
            .map_err(|_| "Could not list workspaces"),
        "list_plugin_actions" if call.arguments.is_empty() => store
            .plugin_actions()
            .map(|actions| json!({"actions": actions}))
            .map_err(|_| "Could not list plugin actions"),
        "list_workspaces" | "list_plugin_actions" => Err("This tool takes no arguments"),
        "invoke_plugin_action" => {
            let arguments: ActionArguments =
                match serde_json::from_value(Value::Object(call.arguments)) {
                    Ok(arguments) => arguments,
                    Err(_) => return Some(tool_error("Invalid plugin action arguments")),
                };
            if arguments.package_sha256.len() != 64 {
                return Some(tool_error("Use a package digest from list_plugin_actions"));
            }
            store.invoke_reviewed_plugin_action(
                &arguments.plugin_id, &arguments.action_id, &arguments.package_sha256,
                arguments.workspace_id, &arguments.input,
            ).map(|output| json!({"output": output}))
             .map_err(|_| "Plugin action could not run. Check the installed revision, scope and permissions")
        }
        _ => generation::call(store, &call.name, call.arguments)?,
    };
    Some(match result {
        Ok(data) => json!({
            "content": [{"type": "text", "text": data.to_string()}],
            "structuredContent": data
        }),
        Err(error) => tool_error(error),
    })
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
        let store = WorkspaceStore::open(":memory:").unwrap();
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
        serve_mcp(&store, requests.trim_start().as_bytes(), &mut output).unwrap();
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
            14
        );
        assert_eq!(
            responses[3]["result"]["structuredContent"]["workspaces"][0]["name"],
            "Studio"
        );
        assert_eq!(responses[4]["result"]["isError"], true);
    }

    #[test]
    fn rejects_overlong_stdio_messages_before_parsing() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let input = vec![b' '; MAX_MESSAGE_BYTES as usize + 1];
        let mut output = Vec::new();
        assert!(serve_mcp(&store, input.as_slice(), &mut output).is_err());
        assert!(output.is_empty());
    }
}
