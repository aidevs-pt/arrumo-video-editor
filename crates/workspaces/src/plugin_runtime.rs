use std::{
    io::{self, Cursor, Read},
    sync::OnceLock,
};

use serde_json::Value;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, Linker},
};

use crate::{PluginManifest, WorkspaceStore};

const MAX_COMPONENT_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_PLUGIN_IO_BYTES: usize = 64 * 1024;
const GUEST_FUEL: u64 = 5_000_000;

impl WorkspaceStore {
    pub fn invoke_plugin_component(&self, plugin_id: &str, input: &Value) -> io::Result<Value> {
        let (manifest, package): (Vec<u8>, Vec<u8>) = self
            .0
            .query_row(
                "SELECT manifest, package_bytes FROM installed_plugin_manifests
                 WHERE plugin_id = ?1 AND package_bytes IS NOT NULL",
                [plugin_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| io::Error::other("Installed plugin package unavailable"))?;
        let manifest = PluginManifest::parse(&manifest)
            .map_err(|_| io::Error::other("Installed plugin manifest invalid"))?;
        if manifest.id != plugin_id {
            return Err(io::Error::other("Installed plugin identity mismatch"));
        }
        let backend = manifest
            .code
            .and_then(|code| code.backend)
            .ok_or_else(|| io::Error::other("Plugin has no backend component"))?;
        let mut archive = zip::ZipArchive::new(Cursor::new(package))
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
        let input = serde_json::to_string(input)
            .map_err(|_| io::Error::other("Could not encode plugin input"))?;
        execute_component(&component, &input)
    }
}

fn execute_component(binary: &[u8], input: &str) -> io::Result<Value> {
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
    let mut store = Store::new(engine, limits);
    store.limiter(|limits: &mut StoreLimits| limits);
    store
        .set_fuel(GUEST_FUEL)
        .map_err(|_| io::Error::other("Could not enforce plugin execution budget"))?;
    let instance = Linker::new(engine)
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
mod tests {
    use super::*;

    const ECHO: &str = r#"
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

    #[test]
    fn executes_component_with_bounded_json() {
        let binary = wat::parse_str(ECHO).unwrap();
        assert_eq!(
            execute_component(&binary, r#"{"hello":"world"}"#).unwrap(),
            serde_json::json!({"hello": "world"})
        );
        assert!(execute_component(&binary, &"0".repeat(MAX_PLUGIN_IO_BYTES + 1)).is_err());
        assert!(execute_component(&binary, "invalid").is_err());
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
            execute_component(&binary, "null")
                .unwrap_err()
                .to_string()
                .contains("isolated runtime")
        );
        let looping = ECHO.replace("i32.const 0))", "(loop $spin (br $spin)) i32.const 0))");
        let binary = wat::parse_str(looping).unwrap();
        assert!(
            execute_component(&binary, "null")
                .unwrap_err()
                .to_string()
                .contains("resource budget")
        );
    }
}
