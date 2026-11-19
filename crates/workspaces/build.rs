use std::{collections::BTreeSet, env, fs, path::PathBuf};

use serde_json::Value;

fn main() {
    for name in [
        "ARRUMO_PLUGIN_REGISTRY",
        "ARRUMO_PLUGIN_SOURCE_ROOT",
        "ARRUMO_CORE_ONLY",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let mut index = String::from("pub(crate) const BUNDLED: &[(&[u8], Option<&[u8]>)] = &[\n");
    if env::var("ARRUMO_CORE_ONLY").as_deref() != Ok("1") {
        let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("crate directory"));
        let root = crate_dir.ancestors().nth(3).expect("Arrumo workspace root");
        let registry = env::var_os("ARRUMO_PLUGIN_REGISTRY")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("plugin-registry/catalog.json"));
        let plugins = env::var_os("ARRUMO_PLUGIN_SOURCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("plugins"));
        let catalog = read_json(&registry);
        assert_eq!(
            catalog["schemaVersion"], 1,
            "unsupported plugin registry schema"
        );
        let mut included = BTreeSet::new();
        for plugin in catalog["plugins"].as_array().expect("registry plugin list") {
            if plugin["default"].as_bool() != Some(true) {
                continue;
            }
            let id = plugin["id"].as_str().expect("registry default plugin ID");
            assert!(
                id.starts_with(|c: char| c.is_ascii_lowercase())
                    && id.split('-').all(|part| !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
                "invalid registry plugin ID"
            );
            assert!(
                included.insert(id),
                "duplicate registry default plugin ID: {id}"
            );
            let directory = plugins.join(id);
            let manifest = directory.join("manifest.json");
            assert_eq!(
                read_json(&manifest)["id"],
                id,
                "registry and source manifest IDs differ"
            );
            let manifest = manifest
                .canonicalize()
                .expect("resolve default plugin manifest");
            let adapter = directory.join("transcription.json");
            println!("cargo:rerun-if-changed={}", directory.display());
            let adapter = if adapter.is_file() {
                let adapter = adapter
                    .canonicalize()
                    .expect("resolve default plugin adapter");
                format!("Some(include_bytes!({adapter:?}))")
            } else {
                "None".into()
            };
            index.push_str(&format!("    (include_bytes!({manifest:?}), {adapter}),\n"));
        }
    }
    index.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::write(output.join("bundled_plugins.rs"), index).expect("write bundled plugin index");
}

fn read_json(path: &std::path::Path) -> Value {
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("Cannot read plugin input {}: {error}. Supply the registry and default plugin checkouts.", path.display())
    });
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("Invalid plugin input {}: {error}", path.display()))
}
