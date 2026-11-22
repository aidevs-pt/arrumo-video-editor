use std::{
    error::Error,
    io::{self, Read, Write},
    path::Path,
};

use serde::Deserialize;
use serde_json::{Map, Value, json};
use subtle::ConstantTimeEq;
use tiny_http::{Header, Method, Request, Response, Server};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{WorkspaceStore, operations};

mod stream;
mod tasks;

const MAX_BODY: u64 = 128 * 1024;

#[derive(Deserialize)]
struct Rpc {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(default = "empty_params")]
    params: Value,
}

fn empty_params() -> Value {
    json!({})
}

#[derive(Deserialize)]
struct SendMessage {
    message: Message,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    role: String,
    message_id: String,
    task_id: Option<String>,
    context_id: Option<String>,
    parts: [Part; 1],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Part {
    data: Operation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    operation: String,
    #[serde(default)]
    arguments: Map<String, Value>,
}

pub fn serve_a2a(
    store: &mut WorkspaceStore,
    database: &Path,
    port: u16,
    token: Zeroizing<Vec<u8>>,
    mut ready: impl Write,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if !(32..=256).contains(&token.len()) || !token.iter().all(u8::is_ascii_alphanumeric) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected a 32–256 character alphanumeric bearer token",
        )
        .into());
    }
    let server = Server::http(("127.0.0.1", port))?;
    let host = server.server_addr().to_string();
    let endpoint = format!("http://{host}/a2a");
    writeln!(ready, "{endpoint}")?;
    ready.flush()?;
    drop(ready);
    let mut streams = Vec::new();
    let mut render_requests = Vec::new();
    for mut request in server.incoming_requests() {
        let version = header(&request, "A2A-Version").map(str::to_owned);
        let (status, body) = match read_request(&token, &host, &endpoint, &mut request) {
            Err(response) => response,
            Ok(body) => {
                let rpc = serde_json::from_slice::<Rpc>(&body).ok();
                if rpc.as_ref().is_some_and(is_motion_render_request) {
                    render_requests
                        .retain(|worker: &std::thread::JoinHandle<()>| !worker.is_finished());
                    if render_requests.len() >= 2 {
                        (
                            503,
                            json!({"error": "Too many active motion render requests"}),
                        )
                    } else {
                        let path = database.to_path_buf();
                        render_requests.push(std::thread::spawn(move || {
                            let (status, response) = match WorkspaceStore::open(&path) {
                                Ok(mut store) => {
                                    (200, dispatch(&mut store, &path, version.as_deref(), &body))
                                }
                                Err(_) => (503, json!({"error": "Workspace store unavailable"})),
                            };
                            respond(request, status, response);
                        }));
                        continue;
                    }
                } else if let Some(rpc) = rpc.filter(|rpc| {
                    matches!(
                        rpc.method.as_str(),
                        "SubscribeToTask" | "SendStreamingMessage"
                    )
                }) {
                    streams.retain(|worker: &std::thread::JoinHandle<()>| !worker.is_finished());
                    if streams.len() >= 8 {
                        (503, json!({"error": "Too many active A2A streams"}))
                    } else if rpc.jsonrpc != "2.0" || !(rpc.id.is_string() || rpc.id.is_number()) {
                        (200, rpc_error(Value::Null, -32600, "Invalid request"))
                    } else if version.as_deref().is_some_and(|version| version != "1.0") {
                        (200, rpc_error(rpc.id, -32009, "Unsupported A2A version"))
                    } else {
                        match stream::prepare(store, database, rpc) {
                            Ok(subscription) => {
                                let path = database.to_path_buf();
                                streams.push(std::thread::spawn(move || {
                                    subscription.send(request, &path)
                                }));
                                continue;
                            }
                            Err(response) => (200, response),
                        }
                    }
                } else {
                    (200, dispatch(store, database, version.as_deref(), &body))
                }
            }
        };
        respond(request, status, body);
    }
    Ok(())
}

fn is_motion_render_request(rpc: &Rpc) -> bool {
    rpc.method == "SendMessage"
        && rpc
            .params
            .pointer("/message/parts/0/data/operation")
            .and_then(Value::as_str)
            == Some("render_run_motion")
}

fn respond(request: Request, status: u16, body: Value) {
    let mut response = Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
    if status == 401 {
        response.add_header(Header::from_bytes("WWW-Authenticate", "Bearer").unwrap());
    }
    let _ = request.respond(response);
}

fn read_request(
    token: &[u8],
    host: &str,
    endpoint: &str,
    request: &mut Request,
) -> Result<Vec<u8>, (u16, Value)> {
    if header(request, "Host") != Some(host) || header(request, "Origin").is_some() {
        return Err((403, json!({"error": "Invalid local request origin"})));
    }
    match (request.method(), request.url()) {
        (&Method::Get, "/.well-known/agent-card.json") => return Err((200, card(endpoint))),
        (&Method::Post, "/a2a") => {}
        _ => return Err((404, json!({"error": "Not found"}))),
    }
    let provided = header(request, "Authorization").and_then(|value| value.strip_prefix("Bearer "));
    if !provided.is_some_and(|value| bool::from(token.ct_eq(value.as_bytes()))) {
        return Err((401, json!({"error": "Authentication required"})));
    }
    if header(request, "Content-Type").and_then(|value| value.split(';').next())
        != Some("application/json")
    {
        return Err((415, json!({"error": "Expected application/json"})));
    }
    let mut body = Vec::new();
    if request
        .as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut body)
        .is_err()
    {
        return Err((400, json!({"error": "Could not read request"})));
    }
    if body.len() as u64 > MAX_BODY {
        return Err((413, json!({"error": "Request exceeds 128 KiB"})));
    }
    Ok(body)
}

fn header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str())
}

fn card(endpoint: &str) -> Value {
    json!({
        "name": "Arrumo Video Editor", "description": "Local workspace and video production operations.",
        "version": env!("CARGO_PKG_VERSION"),
        "supportedInterfaces": [{"url": endpoint, "protocolBinding": "JSONRPC", "protocolVersion": "1.0"}],
        "capabilities": {"streaming": true, "pushNotifications": false},
        "securitySchemes": {"localBearer": {"httpAuthSecurityScheme": {"scheme": "Bearer"}}},
        "securityRequirements": [{"schemes": {"localBearer": {"list": []}}}],
        "defaultInputModes": ["application/json"], "defaultOutputModes": ["application/json"],
        "skills": [{"id": "workspace-operations", "name": "Workspace operations",
            "description": "Send a data part with operation list_operations to discover inputs or invoke a named operation.",
            "tags": ["video", "workspace"]}]
    })
}

fn dispatch(
    store: &mut WorkspaceStore,
    database: &Path,
    version: Option<&str>,
    body: &[u8],
) -> Value {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return rpc_error(Value::Null, -32700, "Invalid JSON");
    };
    let Ok(request) = serde_json::from_value::<Rpc>(value) else {
        return rpc_error(Value::Null, -32600, "Invalid request");
    };
    if request.jsonrpc != "2.0" || !(request.id.is_string() || request.id.is_number()) {
        return rpc_error(Value::Null, -32600, "Invalid request");
    }
    if version.is_some_and(|value| value != "1.0") {
        return rpc_error(request.id, -32009, "Unsupported A2A version");
    }
    if request.method == "ListTasks" {
        return task_reply(request.id, tasks::list(store, request.params), false);
    }
    if matches!(request.method.as_str(), "GetTask" | "CancelTask") {
        return task_reply(
            request.id,
            tasks::handle(store, &request.method, &request.params),
            false,
        );
    }
    if request.method != "SendMessage" {
        return rpc_error(request.id, -32601, "Method not found");
    }
    send_message(store, database, request)
}

fn send_message(store: &mut WorkspaceStore, database: &Path, request: Rpc) -> Value {
    let Ok(input) = serde_json::from_value::<SendMessage>(request.params) else {
        return rpc_error(
            request.id,
            -32602,
            "Expected one structured operation message",
        );
    };
    let Message {
        role,
        message_id,
        task_id,
        context_id,
        parts: [Part { data }],
    } = input.message;
    if role != "ROLE_USER" || message_id.is_empty() {
        return rpc_error(
            request.id,
            -32602,
            "Expected a user message with a message ID",
        );
    }
    if let Some(task_id) = task_id {
        return task_reply(
            request.id,
            review_task(store, database, &task_id, context_id.as_deref(), data),
            true,
        );
    }
    let data = if data.operation == "list_operations" && data.arguments.is_empty() {
        json!({"result": {"operations": operations::definitions()}})
    } else {
        let workspace_id = data.arguments.get("workspaceId").and_then(Value::as_i64);
        let queues_run = matches!(
            data.operation.as_str(),
            "enqueue_video_draft" | "replay_generation"
        );
        match operations::invoke(store, database, &data.operation, data.arguments) {
            Some(Ok(result)) => {
                if let (true, Some(workspace_id), Some(run_id)) =
                    (queues_run, workspace_id, result["run"]["id"].as_i64())
                {
                    return task_reply(
                        request.id,
                        tasks::task_for_run(store, workspace_id, run_id),
                        true,
                    );
                }
                json!({"result": result})
            }
            Some(Err(message)) => json!({"error": message}),
            None => return rpc_error(request.id, -32602, "Unknown operation"),
        }
    };
    json!({"jsonrpc": "2.0", "id": request.id, "result": {"message": {
        "role": "ROLE_AGENT", "messageId": Uuid::new_v4().to_string(), "parts": [{"data": data}]
    }}})
}

fn review_task(
    store: &mut WorkspaceStore,
    database: &Path,
    task_id: &str,
    context_id: Option<&str>,
    mut data: Operation,
) -> tasks::TaskResult {
    let (workspace_id, run_id) = tasks::parse_id(task_id).ok_or((-32001, "Task not found"))?;
    if context_id.is_some_and(|id| id != format!("workspace-{workspace_id}")) {
        return Err((-32602, "Context does not match task"));
    }
    if !matches!(
        data.operation.as_str(),
        "review_production_result" | "review_supplemental_items" | "review_preview_formats"
    ) {
        return Err((-32004, "Task messages support review decisions"));
    }
    for (key, value) in [("workspaceId", workspace_id), ("runId", run_id)] {
        if data
            .arguments
            .get(key)
            .is_some_and(|id| id.as_i64() != Some(value))
        {
            return Err((-32602, "Review scope does not match task"));
        }
        data.arguments.insert(key.to_owned(), json!(value));
    }
    let task = tasks::task_for_run(store, workspace_id, run_id)?;
    if task["status"]["state"] != "TASK_STATE_INPUT_REQUIRED" {
        return Err((-32004, "Task is not waiting for review"));
    }
    match operations::invoke(store, database, &data.operation, data.arguments) {
        Some(Ok(_)) => tasks::task_for_run(store, workspace_id, run_id),
        Some(Err(std::borrow::Cow::Borrowed(message))) => Err((-32602, message)),
        Some(Err(_)) => Err((-32602, "Review operation failed")),
        None => Err((-32603, "Review operation unavailable")),
    }
}

fn task_reply(id: Value, task: tasks::TaskResult, wrap: bool) -> Value {
    match task {
        Ok(task) => {
            let result = if wrap { json!({"task": task}) } else { task };
            json!({"jsonrpc": "2.0", "id": id, "result": result})
        }
        Err((code, message)) => rpc_error(id, code, message),
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_shared_operation_results_as_a2a_messages() {
        let database = Path::new(":memory:");
        let mut store = WorkspaceStore::open(database).unwrap();
        store.create("Studio").unwrap();
        let request = br#"{"jsonrpc":"2.0","id":"fixture","method":"SendMessage","params":{"message":{"messageId":"request","role":"ROLE_USER","parts":[{"data":{"operation":"list_workspaces"}}]}}}"#;
        let reply = dispatch(&mut store, database, Some("1.0"), request);
        assert_eq!(reply["id"], "fixture");
        let message = &reply["result"]["message"];
        assert_eq!(message["role"], "ROLE_AGENT");
        assert_eq!(
            message["parts"][0]["data"]["result"]["workspaces"][0]["name"],
            "Studio"
        );

        let rejected = dispatch(&mut store, database, Some("0.3"), request);
        assert_eq!(rejected["id"], "fixture");
        assert_eq!(rejected["error"]["code"], -32009);
    }
}
