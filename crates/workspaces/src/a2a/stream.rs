use std::{
    io::{self, Write},
    path::Path,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tiny_http::Request;

use super::{Rpc, rpc_error, send_message, tasks};
use crate::WorkspaceStore;

pub(super) struct Subscription {
    id: Value,
    initial: Value,
    task_id: Option<String>,
}

pub(super) fn prepare(
    store: &mut WorkspaceStore,
    database: &Path,
    rpc: Rpc,
) -> Result<Subscription, Value> {
    let id = rpc.id.clone();
    if rpc.method == "SubscribeToTask" {
        let task = tasks::handle(store, "GetTask", &rpc.params)
            .map_err(|(code, message)| rpc_error(id.clone(), code, message))?;
        if is_terminal(&task) {
            return Err(rpc_error(id, -32004, "Task is already terminal"));
        }
        let task_id = task["id"].as_str().map(str::to_owned);
        return Ok(Subscription {
            id,
            initial: json!({"task": task}),
            task_id,
        });
    }
    let mut response = send_message(store, database, rpc);
    if response.get("error").is_some() {
        return Err(response);
    }
    let initial = response["result"].take();
    let task_id = initial["task"]["id"].as_str().map(str::to_owned);
    Ok(Subscription {
        id,
        initial,
        task_id,
    })
}

impl Subscription {
    pub(super) fn send(self, request: Request, database: &Path) {
        let mut writer = request.into_writer();
        let _ = self.write(&mut writer, database);
    }

    fn write(self, writer: &mut impl Write, database: &Path) -> io::Result<()> {
        writer.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nTransfer-Encoding: chunked\r\n\r\n")?;
        write_event(writer, &self.id, &self.initial)?;
        if let Some(task_id) = self.task_id
            && !is_terminal(&self.initial["task"])
        {
            let mut previous = signature(&self.initial["task"]);
            let store = WorkspaceStore::open(database).map_err(io::Error::other)?;
            let query = json!({"id": task_id});
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(60) {
                thread::sleep(Duration::from_secs(1));
                match tasks::handle(&store, "GetTask", &query) {
                    Ok(task) => {
                        let next = signature(&task);
                        let terminal = is_terminal(&task);
                        if next != previous {
                            write_event(writer, &self.id, &json!({"task": task}))?;
                            previous = next;
                        } else {
                            write_chunk(writer, ": heartbeat\n\n")?;
                        }
                        if terminal {
                            break;
                        }
                    }
                    Err((code, message)) => {
                        write_chunk(
                            writer,
                            &format!("data: {}\n\n", rpc_error(self.id, code, message)),
                        )?;
                        break;
                    }
                }
            }
        }
        writer.write_all(b"0\r\n\r\n")?;
        writer.flush()
    }
}

fn signature(task: &Value) -> Value {
    json!([
        task["status"]["state"],
        task["status"]["timestamp"],
        task["status"]["message"]["parts"],
        task["artifacts"]
    ])
}

fn is_terminal(task: &Value) -> bool {
    matches!(
        task["status"]["state"].as_str(),
        Some("TASK_STATE_COMPLETED" | "TASK_STATE_FAILED" | "TASK_STATE_CANCELED")
    )
}

fn write_event(writer: &mut impl Write, id: &Value, result: &Value) -> io::Result<()> {
    let envelope = json!({"jsonrpc": "2.0", "id": id, "result": result});
    write_chunk(writer, &format!("data: {envelope}\n\n"))
}

fn write_chunk(writer: &mut impl Write, data: &str) -> io::Result<()> {
    write!(writer, "{:x}\r\n{data}\r\n", data.len())?;
    writer.flush()
}
