use std::{
    io::{self, Cursor, Read},
    sync::OnceLock,
};

use serde_json::Value;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, Linker},
};

use crate::{PluginActionScope, WorkspaceStore};

const MAX_COMPONENT_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_PLUGIN_IO_BYTES: usize = 64 * 1024;
const GUEST_FUEL: u64 = 5_000_000;

impl WorkspaceStore {
    pub fn invoke_plugin_component(&self, plugin_id: &str, input: &Value) -> io::Result<Value> {
        self.invoke_plugin(plugin_id, None, input, None)
    }

    pub fn invoke_plugin_in_workspace(
        &self,
        plugin_id: &str,
        workspace_id: i64,
        input: &Value,
    ) -> io::Result<Value> {
        self.invoke_plugin(plugin_id, Some(workspace_id), input, None)
    }

    pub(crate) fn invoke_plugin(
        &self,
        plugin_id: &str,
        workspace_id: Option<i64>,
        input: &Value,
        action: Option<(&str, Option<&str>)>,
    ) -> io::Result<Value> {
        let tx = self
            .0
            .unchecked_transaction()
            .map_err(|_| io::Error::other("Could not read plugin invocation state"))?;
        let package =
            self.resolve_plugin_package(plugin_id, action.and_then(|(_, digest)| digest))?;
        let manifest = package.manifest;
        let input = if let Some((action_id, _)) = action {
            let action = manifest
                .actions
                .as_ref()
                .and_then(|actions| actions.iter().find(|action| action.id == action_id))
                .ok_or_else(|| io::Error::other("Plugin action is not declared"))?;
            if (action.scope == PluginActionScope::Workspace) != workspace_id.is_some() {
                return Err(io::Error::other("Plugin action requires a different scope"));
            }
            if let Some(workspace_id) = workspace_id {
                tx.query_row(
                    "SELECT id FROM workspaces WHERE id = ?1",
                    [workspace_id],
                    |_| Ok(()),
                )
                .map_err(|_| io::Error::other("Plugin action workspace unavailable"))?;
            }
            serde_json::to_string(&serde_json::json!({"action": action_id, "input": input}))
        } else {
            if manifest.actions.is_some() {
                return Err(io::Error::other(
                    "Invoke plugins that declare actions by action ID",
                ));
            }
            serde_json::to_string(input)
        }
        .map_err(|_| io::Error::other("Could not encode plugin input"))?;
        let context = if let Some(workspace_id) = workspace_id
            && manifest.backend_requests_workspace_context()
            && self.plugin_context_granted(plugin_id, workspace_id)?
        {
            let value = self
                .workspace_context(workspace_id)
                .map_err(|_| io::Error::other("Could not read granted workspace context"))?;
            let encoded = serde_json::to_string(&value)
                .map_err(|_| io::Error::other("Could not encode granted workspace context"))?;
            if encoded.len() > MAX_PLUGIN_IO_BYTES {
                return Err(io::Error::other(
                    "Workspace context exceeds the plugin transfer limit",
                ));
            }
            Some(encoded)
        } else {
            None
        };
        tx.commit()
            .map_err(|_| io::Error::other("Could not read plugin invocation state"))?;
        let backend = manifest
            .code
            .and_then(|code| code.backend)
            .ok_or_else(|| io::Error::other("Plugin has no backend component"))?;
        let mut archive = zip::ZipArchive::new(Cursor::new(package.bytes))
            .map_err(|_| io::Error::other("Installed plugin ZIP invalid"))?;
        let entry = archive
            .by_name(&backend.component)
            .map_err(|_| io::Error::other("Plugin backend component unavailable"))?;
        if !entry.is_file() || entry.size() > MAX_COMPONENT_BYTES {
            return Err(io::Error::other(
                "Plugin backend component exceeds the size limit",
            ));
        }
        let mut component = Vec::new();
        entry
            .take(MAX_COMPONENT_BYTES + 1)
            .read_to_end(&mut component)
            .map_err(|_| io::Error::other("Could not read plugin backend component"))?;
        execute_component(&component, &input, context.as_deref())
    }
}

struct GuestState {
    limits: StoreLimits,
    context: Option<String>,
}

fn execute_component(binary: &[u8], input: &str, context: Option<&str>) -> io::Result<Value> {
    if binary.len() as u64 > MAX_COMPONENT_BYTES || input.len() > MAX_PLUGIN_IO_BYTES {
        return Err(io::Error::other(
            "Plugin component or input exceeds the size limit",
        ));
    }
    let engine = runtime_engine()?;
    let component = Component::from_binary(engine, binary)
        .map_err(|_| io::Error::other("Invalid plugin WebAssembly component"))?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(16 * 1024 * 1024)
        .table_elements(4096)
        .instances(16)
        .memories(1)
        .tables(4)
        .build();
    let mut store = Store::new(
        engine,
        GuestState {
            limits,
            context: context.map(str::to_owned),
        },
    );
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(GUEST_FUEL)
        .map_err(|_| io::Error::other("Could not enforce plugin execution budget"))?;
    let mut linker = Linker::<GuestState>::new(engine);
    if context.is_some() {
        linker
            .root()
            .func_wrap("read-workspace-context", |mut store, (): ()| {
                let context = store.data_mut().context.take().ok_or_else(|| {
                    wasmtime::Error::msg("Workspace context can be read once per invocation")
                })?;
                Ok((context,))
            })
            .map_err(|_| io::Error::other("Could not link the granted plugin API"))?;
    }
    let instance = linker
        .instantiate(&mut store, &component)
        .map_err(|_| io::Error::other("Plugin could not start in the isolated runtime"))?;
    let invoke = instance
        .get_typed_func::<(&str,), (String,)>(&mut store, "invoke")
        .map_err(|_| {
            io::Error::other("Plugin does not implement the backend invocation contract")
        })?;
    let (output,) = invoke
        .call(&mut store, (input,))
        .map_err(|_| io::Error::other("Plugin execution failed or exceeded its resource budget"))?;
    if output.len() > MAX_PLUGIN_IO_BYTES {
        return Err(io::Error::other("Plugin output exceeds the size limit"));
    }
    serde_json::from_str(&output).map_err(|_| io::Error::other("Plugin output is not valid JSON"))
}

fn runtime_engine() -> io::Result<&'static Engine> {
    static ENGINE: OnceLock<Result<Engine, ()>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut config = Config::new();
            config.consume_fuel(true);
            config.max_wasm_stack(512 * 1024);
            Engine::new(&config).map_err(|_| ())
        })
        .as_ref()
        .map_err(|_| io::Error::other("Could not initialize plugin runtime"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const ECHO: &str = r#"
      (component
        (core module $m
          (memory (export "mem") 2)
          (func (export "alloc") (param i32 i32 i32 i32) (result i32) i32.const 64)
          (func (export "echo") (param $ptr i32) (param $len i32) (result i32)
            (i32.store (i32.const 0) (local.get $ptr))
            (i32.store (i32.const 4) (local.get $len))
            i32.const 0))
        (core instance $i (instantiate $m))
        (alias core export $i "mem" (core memory $mem))
        (alias core export $i "alloc" (core func $alloc))
        (func (export "invoke") (param "input" string) (result string)
          (canon lift (core func $i "echo") (memory $mem) (realloc $alloc))))
    "#;

    const READ_CONTEXT: &str = r#"
      (component
        (import "read-workspace-context" (func $read (result string)))
        (core module $memory
          (memory (export "mem") 2)
          (func (export "alloc") (param i32 i32 i32 i32) (result i32) i32.const 64))
        (core instance $shared (instantiate $memory))
        (alias core export $shared "mem" (core memory $mem))
        (alias core export $shared "alloc" (core func $alloc))
        (core func $lower (canon lower (func $read) (memory $mem) (realloc $alloc)))
        (core module $reader
          (import "host" "read" (func $read (param i32)))
          (func (export "invoke") (param i32 i32) (result i32)
            i32.const 0 call $read i32.const 0))
        (core instance $imports (export "read" (func $lower)))
        (core instance $guest (instantiate $reader (with "host" (instance $imports))))
        (func (export "invoke") (param "input" string) (result string)
          (canon lift (core func $guest "invoke") (memory $mem) (realloc $alloc))))
    "#;

    #[test]
    fn reads_only_the_granted_workspace_and_revokes_after_package_change() {
        use std::io::Write;

        let store = WorkspaceStore::open(":memory:").unwrap();
        let allowed = store.create("Allowed").unwrap().id;
        let other = store.create("Other").unwrap().id;
        store.set_workspace_context(allowed, "Shared").unwrap();
        store.set_workspace_context(other, "Private").unwrap();
        let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        package.start_file("manifest.json", options).unwrap();
        package.write_all(br#"{
            "schemaVersion": 4, "id": "fixture-reader", "name": "Fixture", "description": "Fixture",
            "capabilities": {"reader": {"name": "Read context", "description": "Fixture"}},
            "code": {"backend": {"component": "reader.wasm", "permissions": ["workspace-context-read"]}}
        }"#).unwrap();
        package.start_file("reader.wasm", options).unwrap();
        package
            .write_all(&wat::parse_str(READ_CONTEXT).unwrap())
            .unwrap();
        let bytes = package.finish().unwrap().into_inner();
        store.install_plugin_zip(bytes.as_slice()).unwrap();
        let input = Value::Null;
        let invoke =
            |workspace| store.invoke_plugin_in_workspace("fixture-reader", workspace, &input);
        let reviewed = store
            .plugin_context_requests(allowed)
            .unwrap()
            .into_iter()
            .find(|request| request.plugin_id == "fixture-reader")
            .unwrap();
        assert!(!reviewed.granted);
        assert!(
            store
                .apply_reviewed_plugin_context_grant("fixture-reader", allowed, "stale", true)
                .is_err()
        );
        store
            .apply_reviewed_plugin_context_grant(
                "fixture-reader",
                allowed,
                &reviewed.package_sha256,
                true,
            )
            .unwrap();
        assert_eq!(invoke(allowed).unwrap(), "Shared");
        store
            .apply_reviewed_plugin_context_grant(
                "fixture-reader",
                allowed,
                &reviewed.package_sha256,
                false,
            )
            .unwrap();
        assert!(invoke(allowed).is_err());
        assert!(
            store
                .set_plugin_context_grant("fixture-reader", 999, true)
                .is_err()
        );
        store
            .set_plugin_context_grant("fixture-reader", allowed, true)
            .unwrap();
        assert_eq!(invoke(allowed).unwrap(), "Shared");
        assert!(invoke(other).is_err());
        assert!(
            store
                .invoke_plugin_component("fixture-reader", &input)
                .is_err()
        );
        store
            .set_plugin_context_grant("fixture-reader", allowed, false)
            .unwrap();
        assert!(invoke(allowed).is_err());
        store
            .set_plugin_context_grant("fixture-reader", allowed, true)
            .unwrap();
        store.0.execute(
            "UPDATE installed_plugin_manifests SET package_sha256 = ?1 WHERE plugin_id = 'fixture-reader'",
            ["0".repeat(64)],
        ).unwrap();
        assert!(
            !store
                .plugin_context_granted("fixture-reader", allowed)
                .unwrap()
        );
        assert!(invoke(allowed).is_err());
    }

    #[test]
    fn executes_component_with_bounded_json() {
        let binary = wat::parse_str(ECHO).unwrap();
        assert_eq!(
            execute_component(&binary, r#"{"hello":"world"}"#, None).unwrap(),
            serde_json::json!({"hello": "world"})
        );
        assert!(execute_component(&binary, &"0".repeat(MAX_PLUGIN_IO_BYTES + 1), None).is_err());
        assert!(execute_component(&binary, "invalid", None).is_err());
    }

    #[test]
    fn invokes_only_the_registered_zip_entrypoint() {
        use std::io::Write;

        let store = WorkspaceStore::open(":memory:").unwrap();
        let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
        package
            .start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        package.write_all(br#"{
            "schemaVersion": 3, "id": "fixture-runtime", "name": "Fixture", "description": "Fixture",
            "capabilities": {"echo": {"name": "Echo", "description": "Fixture"}},
            "code": {"backend": {"component": "backend/echo.wasm"}}
        }"#).unwrap();
        package
            .start_file(
                "backend/echo.wasm",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        package.write_all(&wat::parse_str(ECHO).unwrap()).unwrap();
        let bytes = package.finish().unwrap().into_inner();
        let input = serde_json::json!({"items": [1, 2, 3]});
        assert!(
            store
                .invoke_plugin_component("fixture-runtime", &input)
                .is_err()
        );
        store.install_plugin_zip(bytes.as_slice()).unwrap();
        assert_eq!(
            store
                .invoke_plugin_component("fixture-runtime", &input)
                .unwrap(),
            input
        );
        store
            .remove_installed_plugin_manifest("fixture-runtime")
            .unwrap();
        assert!(
            store
                .invoke_plugin_component("fixture-runtime", &input)
                .is_err()
        );
    }

    #[test]
    fn denies_host_imports_and_stops_infinite_work() {
        let imports = ECHO.replace("(component", "(component (import \"host-read\" (func))");
        let binary = wat::parse_str(imports).unwrap();
        assert!(
            execute_component(&binary, "null", None)
                .unwrap_err()
                .to_string()
                .contains("isolated runtime")
        );
        let looping = ECHO.replace("i32.const 0))", "(loop $spin (br $spin)) i32.const 0))");
        let binary = wat::parse_str(looping).unwrap();
        assert!(
            execute_component(&binary, "null", None)
                .unwrap_err()
                .to_string()
                .contains("resource budget")
        );
    }
}
