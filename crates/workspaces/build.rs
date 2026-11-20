use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn main() {
    for name in [
        "ARRUMO_PLUGIN_REGISTRY",
        "ARRUMO_PLUGIN_SOURCE_ROOT",
        "ARRUMO_CORE_ONLY",
        "ARRUMO_PLUGIN_RELEASE_ROOT",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let mut index = String::from("pub(crate) const BUNDLED: &[BundledSource] = &[\n");
    if env::var("ARRUMO_CORE_ONLY").as_deref() != Ok("1") {
        let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("crate directory"));
        let root = crate_dir.ancestors().nth(3).expect("Arrumo workspace root");
        let registry = env::var_os("ARRUMO_PLUGIN_REGISTRY")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("plugin-registry/catalog.json"));
        let plugins = env::var_os("ARRUMO_PLUGIN_SOURCE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("plugins"));
        let release_root = env::var_os("ARRUMO_PLUGIN_RELEASE_ROOT").map(PathBuf::from);
        if let Some(root) = &release_root {
            println!("cargo:rerun-if-changed={}", root.display());
        }
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
            let checkout = read_json(&manifest);
            assert_eq!(
                checkout["id"], id,
                "registry and source manifest IDs differ"
            );
            let manifest = manifest
                .canonicalize()
                .expect("resolve default plugin manifest");
            println!("cargo:rerun-if-changed={}", directory.display());
            let transcription = optional_adapter(&directory, "transcription.json");
            let transcription_timed = optional_adapter(&directory, "transcription-timed.json");
            let speech_synthesis = optional_adapter(&directory, "speech-synthesis.json");
            let (package, source) = default_package(
                &output,
                release_root.as_deref(),
                &directory,
                id,
                plugin,
                &checkout,
            );
            index.push_str(&format!("    BundledSource {{ manifest: include_bytes!({manifest:?}), transcription: {transcription}, transcription_timed: {transcription_timed}, speech_synthesis: {speech_synthesis}, package: {package}, release: {source} }},\n"));
        }
    }
    index.push_str("];\n");
    fs::write(output.join("bundled_plugins.rs"), index).expect("write bundled plugin index");
}

fn optional_adapter(directory: &Path, name: &str) -> String {
    let path = directory.join(name);
    if !path.is_file() {
        return "None".into();
    }
    let path = path.canonicalize().expect("resolve default plugin adapter");
    println!("cargo:rerun-if-changed={}", path.display());
    format!("Some(include_bytes!({path:?}))")
}

fn default_package(
    output: &Path,
    release_root: Option<&Path>,
    checkout_directory: &Path,
    id: &str,
    plugin: &Value,
    checkout: &Value,
) -> (String, String) {
    let release_path = release_root.map(|root| root.join(format!("{id}.zip")));
    let local_path = checkout_directory.join("plugin.zip");
    let debug = env::var("PROFILE").as_deref() == Ok("debug");
    let (bytes, source) = if let Some(path) = release_path.filter(|path| path.is_file()) {
        let bytes = read_package(&path);
        use std::fmt::Write;
        let mut digest = String::with_capacity(64);
        for byte in Sha256::digest(&bytes) {
            write!(&mut digest, "{byte:02x}").expect("write ZIP digest");
        }
        let mut releases = plugin["releases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|release| {
                release["sha256"]
                    .as_str()
                    .is_some_and(|sha| digest.eq_ignore_ascii_case(sha))
            });
        let release = releases
            .next()
            .unwrap_or_else(|| panic!("No registry release matches {id} ZIP digest"));
        assert!(
            releases.next().is_none(),
            "Multiple registry releases match {id} ZIP digest"
        );
        assert_eq!(
            release["manifestSchemaVersion"], checkout["schemaVersion"],
            "registry release schema differs from the default plugin manifest"
        );
        let provenance = json!({
            "pluginId": id,
            "repository": plugin["repository"],
            "version": release["version"],
            "packageUrl": release["packageUrl"],
            "manifestSchemaVersion": release["manifestSchemaVersion"],
        });
        (bytes, format!("Some({:?})", provenance.to_string()))
    } else if debug && local_path.is_file() {
        (read_package(&local_path), "None".into())
    } else {
        return ("None".into(), "None".into());
    };
    let snapshot = output.join(format!("default-{id}.zip"));
    fs::write(&snapshot, bytes).expect("write default plugin ZIP snapshot");
    (format!("Some(include_bytes!({snapshot:?}))"), source)
}

fn read_json(path: &std::path::Path) -> Value {
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!("Cannot read plugin input {}: {error}. Supply the registry and default plugin checkouts.", path.display())
    });
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("Invalid plugin input {}: {error}", path.display()))
}

fn read_package(path: &Path) -> Vec<u8> {
    use std::io::Read;
    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    let path = path.canonicalize().expect("resolve default plugin ZIP");
    println!("cargo:rerun-if-changed={}", path.display());
    let mut bytes = Vec::new();
    fs::File::open(path)
        .expect("open default plugin ZIP")
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .expect("read default plugin ZIP");
    assert!(
        bytes.len() as u64 <= MAX_BYTES,
        "default plugin ZIP exceeds 16 MiB"
    );
    bytes
}
