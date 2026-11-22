use std::{
    collections::BTreeSet,
    env, fs,
    io::Read,
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
    let debug = env::var("PROFILE").as_deref() == Ok("debug");
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
            let (package, source, verified_zip) = default_package(
                &output,
                release_root.as_deref(),
                &directory,
                id,
                plugin,
                &checkout,
            );
            let mut verified_zip = verified_zip.map(|snapshot| {
                let archive =
                    fs::File::open(snapshot).expect("open verified default plugin snapshot");
                let mut archive =
                    zip::ZipArchive::new(archive).expect("read verified default plugin ZIP");
                assert!(
                    archive.len() <= 512,
                    "Verified default plugin ZIP exceeds 512 entries"
                );
                archive
                    .validate_header_consistency()
                    .expect("verify default ZIP headers");
                let bytes = read_release_entry(id, &mut archive, "manifest.json")
                    .expect("verified default plugin ZIP must contain a root manifest");
                let packaged: Value =
                    serde_json::from_slice(&bytes).expect("parse verified manifest");
                assert_eq!(
                    packaged, checkout,
                    "Verified {id} manifest differs from the checkout"
                );
                archive
            });
            let mut adapter = |name| match &mut verified_zip {
                Some(archive) => release_adapter(&output, id, archive, name),
                None if debug && release_root.is_none() => optional_adapter(&directory, name),
                None => "None".into(),
            };
            let transcription = adapter("transcription.json");
            let transcription_timed = adapter("transcription-timed.json");
            let speech_synthesis = adapter("speech-synthesis.json");
            let image_generation = adapter("image-generation.json");
            let video_generation = adapter("video-generation.json");
            let usage = adapter("usage.json");
            let audio_generation = adapter("audio-generation.json");
            let audio_transform = adapter("audio-transform.json");
            let icon = match (checkout["icon"].as_str(), &mut verified_zip) {
                (None, _) => "None".into(),
                (Some(name), Some(archive)) => release_adapter(&output, id, archive, name),
                (Some(name), None) => optional_adapter(&directory, name),
            };
            index.push_str(&format!("    BundledSource {{ manifest: include_bytes!({manifest:?}), transcription: {transcription}, transcription_timed: {transcription_timed}, speech_synthesis: {speech_synthesis}, image_generation: {image_generation}, video_generation: {video_generation}, audio_generation: {audio_generation}, audio_transform: {audio_transform}, usage: {usage}, icon: {icon}, package: {package}, release: {source} }},\n"));
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

fn release_adapter(
    output: &Path,
    id: &str,
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
) -> String {
    let Some(bytes) = read_release_entry(id, archive, name) else {
        return "None".into();
    };
    let snapshot = output.join(format!("default-{id}-{}", name.replace('/', "-")));
    fs::write(&snapshot, bytes).expect("write verified adapter snapshot");
    format!("Some(include_bytes!({snapshot:?}))")
}
fn read_release_entry(
    id: &str,
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
) -> Option<Vec<u8>> {
    const MAX_ENTRY_BYTES: u64 = 64 * 1024;
    let matches = (0..archive.len())
        .filter(|index| {
            let entry = archive
                .by_index_raw(*index)
                .expect("read verified ZIP entry");
            entry.name_raw() == name.as_bytes()
        })
        .count();
    if matches == 0 {
        return None;
    }
    assert_eq!(matches, 1, "Duplicate {id} entry {name}");
    let entry = archive.by_name(name).expect("open verified ZIP entry");
    assert!(
        entry.is_file() && !entry.is_symlink() && entry.size() <= MAX_ENTRY_BYTES,
        "Invalid {id} entry {name}"
    );
    let mut bytes = Vec::new();
    entry
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut bytes)
        .expect("read verified ZIP entry");
    assert!(
        bytes.len() as u64 <= MAX_ENTRY_BYTES,
        "Verified ZIP entry exceeds 64 KiB"
    );
    Some(bytes)
}

fn default_package(
    output: &Path,
    release_root: Option<&Path>,
    checkout_directory: &Path,
    id: &str,
    plugin: &Value,
    checkout: &Value,
) -> (String, String, Option<PathBuf>) {
    let release_path = release_root.map(|root| root.join(format!("{id}.zip")));
    let local_path = checkout_directory.join("plugin.zip");
    let debug = env::var("PROFILE").as_deref() == Ok("debug");
    let (bytes, source, verified) = if let Some(path) = release_path.filter(|path| path.is_file()) {
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
        (bytes, format!("Some({:?})", provenance.to_string()), true)
    } else if release_root.is_none() && debug && local_path.is_file() {
        (read_package(&local_path), "None".into(), false)
    } else {
        return ("None".into(), "None".into(), None);
    };
    let snapshot = output.join(format!("default-{id}.zip"));
    fs::write(&snapshot, bytes).expect("write default plugin ZIP snapshot");
    (
        format!("Some(include_bytes!({snapshot:?}))"),
        source,
        verified.then_some(snapshot),
    )
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
