//! Canvas layers: animation layers an agent writes as a small web page that draws one frame for
//! any time it is given, with WebGL, canvas, SVG, CSS and the libraries a canvas renderer plugin
//! ships. The plugin names a headless browser for each platform; Arrumo downloads it only when the
//! creator asks, checks its digest, and drives it over a pipe with the DevTools protocol. Pages
//! get no network, no files and no app access: every request is answered from memory or refused,
//! and on Linux the browser also runs inside Bubblewrap without a network.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt::Write as _,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::WorkspaceStore;

/// The most page source one canvas layer carries.
pub(crate) const MAX_CANVAS_HTML: usize = 48 * 1024;
const MAX_LIBRARY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GUIDE_BYTES: u64 = 32 * 1024;
const ORIGIN: &str = "https://canvas.arrumo";
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasRendererDescriptor {
    pub browser: BTreeMap<String, BrowserDownload>,
    pub libraries: Vec<CanvasLibrary>,
    pub guide: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserDownload {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub executable: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasLibrary {
    pub id: String,
    pub version: String,
    pub file: String,
    pub module: bool,
    pub license: String,
    pub description: String,
}

const PLATFORMS: [&str; 5] = [
    "linux-x64",
    "linux-arm64",
    "macos-arm64",
    "macos-x64",
    "windows-x64",
];

fn package_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
}

impl CanvasRendererDescriptor {
    pub(crate) fn valid(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        !self.browser.is_empty()
            && self.browser.iter().all(|(platform, download)| {
                PLATFORMS.contains(&platform.as_str())
                    && download.url.starts_with("https://")
                    && download.url.ends_with(".zip")
                    && !download.url.chars().any(char::is_whitespace)
                    && crate::plugin_package::valid_sha256(&download.sha256)
                    && download.sha256.bytes().all(|b| !b.is_ascii_uppercase())
                    && (1..=512 * 1024 * 1024).contains(&download.size)
                    && !download.executable.is_empty()
                    && download.executable.len() <= 200
                    && !download.executable.starts_with('/')
                    && download
                        .executable
                        .split('/')
                        .all(|part| !part.is_empty() && part != "." && part != "..")
            })
            && self.libraries.len() <= 16
            && self.libraries.iter().all(|library| {
                let id = library.id.as_bytes();
                (1..=32).contains(&id.len())
                    && id[0].is_ascii_lowercase()
                    && id
                        .iter()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
                    && ids.insert(library.id.clone())
                    && (1..=32).contains(&library.version.len())
                    && package_path(&library.file)
                    && (library.file.ends_with(".js") || library.file.ends_with(".mjs"))
                    && (1..=64).contains(&library.license.len())
                    && !library.description.trim().is_empty()
                    && library.description.chars().count() <= 240
            })
            && package_path(&self.guide)
            && self.guide.ends_with(".md")
    }
}

/// The platform key a plugin names its browser download under.
pub(crate) fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x64"),
        ("linux", "aarch64") => Some("linux-arm64"),
        ("macos", "aarch64") => Some("macos-arm64"),
        ("macos", "x86_64") => Some("macos-x64"),
        ("windows", "x86_64") => Some("windows-x64"),
        _ => None,
    }
}

#[derive(Clone)]
pub(crate) struct Library {
    pub id: String,
    pub module: bool,
    pub description: String,
    bytes: Arc<Vec<u8>>,
}

#[derive(Clone)]
pub(crate) struct CanvasRuntime {
    pub browser: PathBuf,
    pub libraries: Vec<Library>,
    pub guide: String,
    bubblewrap: Option<PathBuf>,
}

static ACTIVE: Mutex<Option<Arc<CanvasRuntime>>> = Mutex::new(None);
/// One download at a time; a second request waits and then finds the browser in place.
static INSTALLING: Mutex<()> = Mutex::new(());
static DOWNLOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The renderer canvas layers are drawn with in this process, when one is ready.
pub(crate) fn active() -> Option<Arc<CanvasRuntime>> {
    #[cfg(test)]
    if let Some(runtime) = TEST_ACTIVE.with(|active| active.borrow().clone()) {
        return Some(runtime);
    }
    ACTIVE.lock().ok()?.clone()
}

#[cfg(test)]
thread_local! {
    static TEST_ACTIVE: std::cell::RefCell<Option<Arc<CanvasRuntime>>> = const { std::cell::RefCell::new(None) };
}

/// Readies canvas layers from the first saved database this process opens; turning plugins on or
/// off and downloading the browser refresh it after that.
pub(crate) fn load_once(store: &WorkspaceStore) {
    static LOADED: std::sync::Once = std::sync::Once::new();
    if store.canvas_root().is_some() {
        LOADED.call_once(|| {
            let _ = store.canvas_status();
        });
    }
}

fn set_active(runtime: Option<CanvasRuntime>) {
    if let Ok(mut active) = ACTIVE.lock() {
        *active = runtime.map(Arc::new);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasLibraryInfo {
    pub id: String,
    pub version: String,
    pub license: String,
}

/// Whether agents can draw canvas layers here, and what is missing when they can't.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasStatus {
    pub plugin_id: Option<String>,
    pub plugin_name: Option<String>,
    /// Whether the plugin names a browser for this platform.
    pub supported: bool,
    pub download_bytes: Option<u64>,
    pub installed: bool,
    /// How pages are isolated: `browser` (the browser's own sandbox) or `bubblewrap`.
    pub sandbox: Option<&'static str>,
    pub ready: bool,
    /// Whether this process is downloading the browser now.
    pub downloading: bool,
    /// Why a supported, installed renderer still can't run.
    pub problem: Option<String>,
    pub libraries: Vec<CanvasLibraryInfo>,
}

fn read_entry(bytes: &[u8], name: &str, limit: u64) -> io::Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(bytes))
        .map_err(|_| io::Error::other("Bad package"))?;
    let entry = archive
        .by_name(name)
        .map_err(|_| io::Error::other(format!("The plugin package lacks {name}")))?;
    if !entry.is_file() || entry.size() > limit {
        return Err(io::Error::other(format!("{name} is too large")));
    }
    let mut data = Vec::new();
    entry.take(limit + 1).read_to_end(&mut data)?;
    Ok(data)
}

/// Checks the guide and libraries a package's canvas renderer names are in it.
pub(crate) fn check_package(bytes: &[u8], canvas: &CanvasRendererDescriptor) -> io::Result<()> {
    String::from_utf8(read_entry(bytes, &canvas.guide, MAX_GUIDE_BYTES)?)
        .map_err(|_| io::Error::other("The canvas guide is not UTF-8"))?;
    for library in &canvas.libraries {
        read_entry(bytes, &library.file, MAX_LIBRARY_BYTES)?;
    }
    Ok(())
}

fn bubblewrap() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    ["/usr/bin/bwrap", "/bin/bwrap"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

impl WorkspaceStore {
    fn canvas_root(&self) -> Option<PathBuf> {
        let path = self.0.path().filter(|path| !path.is_empty())?;
        let mut root = PathBuf::from(path).into_os_string();
        root.push(".canvas");
        Some(PathBuf::from(root))
    }

    fn canvas_plugin(
        &self,
    ) -> Result<Option<(crate::CatalogPlugin, CanvasRendererDescriptor)>, Box<dyn Error>> {
        Ok(self
            .active_plugin_catalog()?
            .into_iter()
            .find_map(|plugin| {
                let canvas = plugin.manifest.canvas_renderer.clone()?;
                Some((plugin, canvas))
            }))
    }

    /// Reports whether canvas layers can be drawn and makes the renderer this process uses
    /// match it.
    pub fn canvas_status(&self) -> Result<CanvasStatus, Box<dyn Error>> {
        let mut status = CanvasStatus {
            plugin_id: None,
            plugin_name: None,
            supported: false,
            download_bytes: None,
            installed: false,
            sandbox: None,
            ready: false,
            downloading: DOWNLOADING.load(std::sync::atomic::Ordering::Relaxed),
            problem: None,
            libraries: Vec::new(),
        };
        let Some((plugin, canvas)) = self.canvas_plugin()? else {
            set_active(None);
            return Ok(status);
        };
        status.plugin_id = Some(plugin.manifest.id.clone());
        status.plugin_name = Some(plugin.manifest.name.clone());
        status.libraries = canvas
            .libraries
            .iter()
            .map(|library| CanvasLibraryInfo {
                id: library.id.clone(),
                version: library.version.clone(),
                license: library.license.clone(),
            })
            .collect();
        let download = platform().and_then(|platform| canvas.browser.get(platform));
        status.supported = download.is_some();
        status.download_bytes = download.map(|download| download.size);
        let (Some(download), Some(root)) = (download, self.canvas_root()) else {
            set_active(None);
            return Ok(status);
        };
        let browser = root.join(&download.sha256).join(&download.executable);
        status.installed = browser.is_file();
        if !status.installed {
            set_active(None);
            return Ok(status);
        }
        let bubblewrap = bubblewrap();
        if cfg!(target_os = "linux") && bubblewrap.is_none() {
            status.problem = Some(
                "Canvas layers on Linux need Bubblewrap (the bwrap command) to keep pages isolated."
                    .into(),
            );
            set_active(None);
            return Ok(status);
        }
        status.sandbox = Some(if bubblewrap.is_some() {
            "bubblewrap"
        } else {
            "browser"
        });
        let Ok(package) = self.resolve_plugin_package(&plugin.manifest.id, None) else {
            status.problem = Some("The canvas renderer plugin's package is unavailable.".into());
            set_active(None);
            return Ok(status);
        };
        let guide = String::from_utf8(read_entry(&package.bytes, &canvas.guide, MAX_GUIDE_BYTES)?)?;
        let mut libraries = Vec::new();
        for library in &canvas.libraries {
            libraries.push(Library {
                id: library.id.clone(),
                module: library.module,
                description: library.description.clone(),
                bytes: Arc::new(read_entry(
                    &package.bytes,
                    &library.file,
                    MAX_LIBRARY_BYTES,
                )?),
            });
        }
        status.ready = true;
        set_active(Some(CanvasRuntime {
            browser,
            libraries,
            guide,
            bubblewrap,
        }));
        Ok(status)
    }

    /// Downloads the browser the active canvas renderer names for this platform, checks its
    /// size and SHA-256, and unpacks it next to the database.
    pub fn install_canvas_browser(&self) -> Result<CanvasStatus, Box<dyn Error>> {
        let (_, canvas) = self
            .canvas_plugin()?
            .ok_or_else(|| io::Error::other("No canvas renderer plugin is turned on"))?;
        let download = platform()
            .and_then(|platform| canvas.browser.get(platform))
            .ok_or_else(|| {
                io::Error::other("The canvas renderer has no browser for this computer")
            })?
            .clone();
        let root = self
            .canvas_root()
            .ok_or_else(|| io::Error::other("Canvas layers need a saved workspace database"))?;
        let target = root.join(&download.sha256);
        let _installing = INSTALLING
            .lock()
            .map_err(|_| io::Error::other("Canvas download failed"))?;
        if !target.join(&download.executable).is_file() {
            DOWNLOADING.store(true, std::sync::atomic::Ordering::Relaxed);
            struct Done;
            impl Drop for Done {
                fn drop(&mut self) {
                    DOWNLOADING.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            }
            let _done = Done;
            fs::create_dir_all(&root)?;
            let mut archive = tempfile::tempfile_in(&root)?;
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30 * 60))
                .build()?;
            let mut response = client.get(&download.url).send()?.error_for_status()?;
            let mut hasher = Sha256::new();
            let mut buffer = vec![0; 256 * 1024];
            let mut total = 0u64;
            loop {
                let read = response.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                total += read as u64;
                if total > download.size {
                    return Err(
                        io::Error::other("The browser download is larger than declared").into(),
                    );
                }
                hasher.update(&buffer[..read]);
                archive.write_all(&buffer[..read])?;
            }
            let digest: String = hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            if total != download.size || digest != download.sha256 {
                return Err(
                    io::Error::other("The browser download didn't match its checksum").into(),
                );
            }
            let partial = root.join(format!("{}.partial", download.sha256));
            let _ = fs::remove_dir_all(&partial);
            unpack(archive, &partial)?;
            if !partial.join(&download.executable).is_file() {
                let _ = fs::remove_dir_all(&partial);
                return Err(io::Error::other("The browser download lacks its program").into());
            }
            let _ = fs::remove_dir_all(&target);
            fs::rename(&partial, &target)?;
        }
        // Earlier browsers this renderer named are no longer used.
        for entry in fs::read_dir(&root)?.flatten() {
            if entry.file_name() != download.sha256.as_str() && entry.path().is_dir() {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
        self.canvas_status()
    }

    /// Downloads the browser when a canvas renderer plugin is on and its browser for this
    /// computer is missing, so canvas layers work without the creator asking.
    pub fn ensure_canvas_browser(&self) -> Result<CanvasStatus, Box<dyn Error>> {
        let status = self.canvas_status()?;
        if status.supported && !status.installed {
            return self.install_canvas_browser();
        }
        Ok(status)
    }

    /// Removes downloaded canvas browsers.
    pub fn remove_canvas_browser(&self) -> Result<CanvasStatus, Box<dyn Error>> {
        let _installing = INSTALLING
            .lock()
            .map_err(|_| io::Error::other("Canvas removal failed"))?;
        if let Some(root) = self.canvas_root()
            && root.is_dir()
        {
            fs::remove_dir_all(root)?;
        }
        self.canvas_status()
    }
}

fn unpack(archive: fs::File, into: &Path) -> io::Result<()> {
    let mut archive = zip::ZipArchive::new(archive).map_err(io::Error::other)?;
    fs::create_dir_all(into)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(io::Error::other)?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(io::Error::other("The browser download has an unsafe path"));
        };
        let path = into.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if entry.is_symlink() {
            let mut target = String::new();
            entry.read_to_string(&mut target)?;
            let resolved = path.parent().unwrap_or(into).join(&target);
            let escapes = Path::new(&target).is_absolute()
                || !normalize(&resolved).starts_with(normalize(into));
            if escapes {
                return Err(io::Error::other("The browser download has an unsafe link"));
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &path)?;
            continue;
        }
        let mut file = fs::File::create(&path)?;
        io::copy(&mut entry, &mut file)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o755))?;
        }
    }
    Ok(())
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// A canvas layer's page and what it is drawn with.
pub(crate) struct CanvasPage<'a> {
    pub html: &'a str,
    pub libraries: &'a [String],
    pub values: &'a serde_json::Map<String, Value>,
    /// Font files served to the page by family name.
    pub fonts: &'a [(&'a str, &'a str, PathBuf)],
    /// The colours the page is themed with: `colors` (the brand palette), `background`, `text`
    /// and `accent`, each a hex colour.
    pub theme: &'a Value,
    /// Workspace components the page may import as `workspace/<slug>`.
    pub components: &'a [crate::canvas_components::CanvasComponent],
}

impl CanvasPage<'_> {
    /// The libraries the page and the components it imports use.
    fn uses(&self, library: &str) -> bool {
        self.libraries.iter().any(|id| id == library)
            || self.components.iter().any(|component| {
                component.libraries.iter().any(|id| id == library)
                    && self.html.contains(&component.import_path())
            })
    }
}

fn is_hex_color(value: &str) -> bool {
    let digits = value.strip_prefix('#').unwrap_or("");
    matches!(digits.len(), 3 | 4 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
}

/// The theme as page settings and CSS variables, keeping only well-formed colours.
fn theme_parts(theme: &Value) -> (Value, String) {
    let color = |key: &str| theme[key].as_str().filter(|value| is_hex_color(value));
    let colors: Vec<&str> = theme["colors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| is_hex_color(value))
        .take(12)
        .collect();
    let mut settings = serde_json::Map::new();
    settings.insert("colors".into(), json!(colors));
    let mut css = String::new();
    for key in ["background", "text", "accent"] {
        if let Some(value) = color(key) {
            settings.insert(key.into(), json!(value));
            write!(css, "--arrumo-{key}:{value};").ok();
        }
    }
    for (index, value) in colors.iter().enumerate() {
        write!(css, "--arrumo-color-{}:{value};", index + 1).ok();
    }
    (Value::Object(settings), format!(":root{{{css}}}"))
}

/// Runs before the page's own scripts: time only moves when Arrumo draws a frame, so animation
/// loops, CSS and SVG animations land on the same frame every render.
const CLOCK: &str = r#"(() => {
  let now = 0, stepping = false, queue = [], next = 1;
  const realFrame = window.requestAnimationFrame.bind(window);
  const epoch = Date.UTC(2026, 0, 1);
  performance.now = () => now;
  Date.now = () => epoch + now;
  window.requestAnimationFrame = (callback) => {
    const id = next++;
    if (stepping) queue.push([id, callback]); else realFrame(() => callback(now));
    return id;
  };
  window.cancelAnimationFrame = (id) => { queue = queue.filter((entry) => entry[0] !== id); };
  window.__arrumoReady = async () => {
    if (document.readyState !== "complete") await new Promise((done) => addEventListener("load", done, { once: true }));
    await document.fonts.ready;
    await Promise.resolve(window.arrumoReady);
    stepping = true;
    return typeof window.render === "function" || document.getAnimations().length > 0 || !!document.querySelector("svg");
  };
  window.__arrumoStep = async (t) => {
    now = t * 1000;
    const callbacks = queue; queue = [];
    for (const [, callback] of callbacks) callback(now);
    if (typeof window.render === "function") await window.render(t);
    for (const animation of document.getAnimations()) { animation.pause(); animation.currentTime = now; }
    for (const svg of document.querySelectorAll("svg")) if (svg.pauseAnimations) { svg.pauseAnimations(); svg.setCurrentTime(t); }
  };
})();"#;

struct Session {
    child: Child,
    to_browser: Box<dyn Write + Send>,
    messages: mpsc::Receiver<Value>,
    next_id: u64,
    session: Option<String>,
    resources: BTreeMap<String, (&'static str, Arc<Vec<u8>>)>,
    errors: Vec<String>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Session {
    fn send(&mut self, method: &str, params: Value) -> io::Result<u64> {
        self.next_id += 1;
        let mut message = json!({"id": self.next_id, "method": method, "params": params});
        if let Some(session) = &self.session {
            message["sessionId"] = json!(session);
        }
        let mut bytes = serde_json::to_vec(&message)?;
        bytes.push(0);
        self.to_browser.write_all(&bytes)?;
        self.to_browser.flush()?;
        Ok(self.next_id)
    }

    fn call(&mut self, method: &str, params: Value, timeout: Duration) -> io::Result<Value> {
        let id = self.send(method, params)?;
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = self.messages.recv_timeout(left).map_err(|_| {
                io::Error::other(match self.errors.first() {
                    Some(error) => format!("The canvas page stopped responding: {error}"),
                    None => "The canvas page took too long to draw".to_owned(),
                })
            })?;
            if message["id"].as_u64() == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(io::Error::other(format!(
                        "The canvas browser refused {method}: {}",
                        error["message"].as_str().unwrap_or("unknown error")
                    )));
                }
                return Ok(message["result"].clone());
            }
            self.event(&message)?;
        }
    }

    fn event(&mut self, message: &Value) -> io::Result<()> {
        let params = &message["params"];
        match message["method"].as_str() {
            Some("Fetch.requestPaused") => {
                let request = params["requestId"].clone();
                let url = params["request"]["url"].as_str().unwrap_or_default();
                let path = url
                    .strip_prefix(ORIGIN)
                    .map(|path| path.split(['?', '#']).next().unwrap_or_default());
                match path.and_then(|path| self.resources.get(path)).cloned() {
                    Some((kind, body)) => {
                        self.send(
                            "Fetch.fulfillRequest",
                            json!({
                                "requestId": request,
                                "responseCode": 200,
                                "responseHeaders": [{"name": "Content-Type", "value": kind}],
                                "body": base64::engine::general_purpose::STANDARD.encode(body.as_slice()),
                            }),
                        )?;
                    }
                    None => {
                        self.send(
                            "Fetch.failRequest",
                            json!({"requestId": request, "errorReason": "BlockedByClient"}),
                        )?;
                    }
                }
            }
            Some("Runtime.exceptionThrown") if self.errors.len() < 5 => {
                let details = &params["exceptionDetails"];
                let text = details["exception"]["description"]
                    .as_str()
                    .or(details["text"].as_str())
                    .unwrap_or("error");
                self.errors.push(
                    text.lines()
                        .next()
                        .unwrap_or(text)
                        .chars()
                        .take(300)
                        .collect(),
                );
            }
            _ => {}
        }
        Ok(())
    }

    fn evaluate(&mut self, expression: &str, timeout: Duration) -> io::Result<Value> {
        let result = self.call(
            "Runtime.evaluate",
            json!({"expression": expression, "awaitPromise": true, "returnByValue": true}),
            timeout,
        )?;
        if let Some(details) = result.get("exceptionDetails") {
            let text = details["exception"]["description"]
                .as_str()
                .or(details["text"].as_str())
                .unwrap_or("error");
            return Err(io::Error::other(format!(
                "The canvas page failed: {}",
                text.lines().next().unwrap_or(text)
            )));
        }
        Ok(result["result"]["value"].clone())
    }
}

fn launch(runtime: &CanvasRuntime, profile: &Path) -> io::Result<Session> {
    let browser_flags = [
        "--headless",
        "--remote-debugging-pipe",
        "--no-first-run",
        "--no-default-browser-check",
        "--mute-audio",
        "--hide-scrollbars",
        "--disable-extensions",
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-sync",
        "--force-device-scale-factor=1",
        "--use-angle=swiftshader",
        "--enable-unsafe-swiftshader",
        "--host-resolver-rules=MAP * ~NOTFOUND",
        "--proxy-server=127.0.0.1:9",
    ];
    let (read_commands, write_commands) = io::pipe()?;
    let (read_replies, write_replies) = io::pipe()?;
    let mut command = match &runtime.bubblewrap {
        Some(bwrap) => {
            let directory = runtime
                .browser
                .parent()
                .ok_or_else(|| io::Error::other("Bad browser path"))?;
            let mut command = crate::quiet_command(bwrap);
            command.args([
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--clearenv",
            ]);
            for path in [
                "/usr",
                "/etc/fonts",
                "/etc/ld.so.cache",
                "/etc/alternatives",
            ] {
                if Path::new(path).exists() {
                    command.args(["--ro-bind", path, path]);
                }
            }
            for (link, target) in [
                ("/lib", "usr/lib"),
                ("/lib64", "usr/lib64"),
                ("/bin", "usr/bin"),
            ] {
                if Path::new(link).is_symlink() {
                    command.args(["--symlink", target, link]);
                } else if Path::new(link).exists() {
                    command.args(["--ro-bind", link, link]);
                }
            }
            command
                .arg("--ro-bind")
                .arg(directory)
                .arg("/browser")
                .args(["--tmpfs", "/tmp", "--proc", "/proc", "--dev", "/dev"])
                .args(["--setenv", "HOME", "/tmp", "--setenv", "TMPDIR", "/tmp"])
                .arg(Path::new("/browser").join(runtime.browser.file_name().unwrap_or_default()))
                .args(browser_flags)
                .args(["--no-sandbox", "--user-data-dir=/tmp/profile"]);
            command
        }
        None => {
            let mut command = crate::quiet_command(&runtime.browser);
            command
                .args(browser_flags)
                .arg(format!("--user-data-dir={}", profile.display()));
            command
        }
    };
    command
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    pass_pipes(&mut command, &read_commands, &write_replies)?;
    let child = command.spawn()?;
    drop(read_commands);
    drop(write_replies);
    let (sender, messages) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(read_replies);
        let mut buffer = Vec::new();
        while reader.read_until(0, &mut buffer).is_ok_and(|read| read > 0) {
            if buffer.last() == Some(&0) {
                buffer.pop();
            }
            if let Ok(message) = serde_json::from_slice::<Value>(&buffer)
                && sender.send(message).is_err()
            {
                break;
            }
            buffer.clear();
        }
    });
    Ok(Session {
        child,
        to_browser: Box::new(write_commands),
        messages,
        next_id: 0,
        session: None,
        resources: BTreeMap::new(),
        errors: Vec::new(),
    })
}

/// Hands the browser the pipe ends it talks over: descriptors 3 and 4 on Unix, inherited
/// handles named on the command line on Windows.
#[cfg(unix)]
fn pass_pipes(
    command: &mut Command,
    read: &io::PipeReader,
    write: &io::PipeWriter,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    let (read, write) = (read.as_raw_fd(), write.as_raw_fd());
    // SAFETY: dup2 is async-signal-safe and only rearranges descriptors in the child.
    unsafe {
        command.pre_exec(move || {
            // Move both ends clear of 3 and 4 first, so one dup2 can't close the other.
            let (read, write) = (libc::dup(read), libc::dup(write));
            if read < 0 || write < 0 || libc::dup2(read, 3) < 0 || libc::dup2(write, 4) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(())
}

#[cfg(windows)]
fn pass_pipes(
    command: &mut Command,
    read: &io::PipeReader,
    write: &io::PipeWriter,
) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
    let (read, write) = (read.as_raw_handle(), write.as_raw_handle());
    for handle in [read, write] {
        // SAFETY: the handles stay open until the child has started.
        if unsafe { SetHandleInformation(handle as _, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    command.arg(format!(
        "--remote-debugging-io-pipes={},{}",
        read as usize, write as usize
    ));
    Ok(())
}

fn page_document(
    page: &CanvasPage,
    runtime: &CanvasRuntime,
    size: (u32, u32),
    fps: u32,
    duration: f64,
) -> String {
    let (theme, variables) = theme_parts(page.theme);
    let setup = json!({
        "width": size.0,
        "height": size.1,
        "fps": fps,
        "duration": duration,
        "values": page.values,
        "theme": theme,
    });
    let imports: serde_json::Map<String, Value> = runtime
        .libraries
        .iter()
        .filter(|library| library.module && page.uses(&library.id))
        .map(|library| {
            (
                library.id.clone(),
                json!(format!("{ORIGIN}/lib/{}.js", library.id)),
            )
        })
        .chain(page.components.iter().map(|component| {
            (
                component.import_path(),
                json!(format!("{ORIGIN}/{}.js", component.import_path())),
            )
        }))
        .collect();
    let scripts: String = runtime
        .libraries
        .iter()
        .filter(|library| !library.module && page.uses(&library.id))
        .map(|library| format!("<script src=\"{ORIGIN}/lib/{}.js\"></script>", library.id))
        .collect();
    let fonts: String = page
        .fonts
        .iter()
        .map(|(family, weight, _)| {
            format!(
                "@font-face{{font-family:\"{family}\";font-weight:{weight};src:url(\"{ORIGIN}/fonts/{family}-{weight}\")}}"
            )
        })
        .collect();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <style>{variables}{fonts}html,body{{margin:0;padding:0;background:transparent;overflow:hidden;width:{w}px;height:{h}px}}</style>\
         <script>window.arrumo={setup};</script>\
         <script type=\"importmap\">{{\"imports\":{imports}}}</script>{scripts}</head>\
         <body>{html}</body></html>",
        w = size.0,
        h = size.1,
        imports = Value::Object(imports),
        html = page.html,
    )
}

/// Draws `frames` frames of a canvas page at `size`, `fps` frames a second, into PNG files named
/// `f00000.png` onwards in `output`, with transparent pixels where the page draws nothing.
pub(crate) fn render_frames(
    runtime: &CanvasRuntime,
    page: &CanvasPage,
    size: (u32, u32),
    fps: u32,
    frames: u32,
    output: &Path,
) -> io::Result<()> {
    let profile = tempfile::tempdir()?;
    let mut session = launch(runtime, profile.path())?;
    let duration = f64::from(frames) / f64::from(fps);
    let document = page_document(page, runtime, size, fps, duration);
    session.resources.insert(
        "/page.html".into(),
        ("text/html; charset=utf-8", Arc::new(document.into_bytes())),
    );
    for library in &runtime.libraries {
        session.resources.insert(
            format!("/lib/{}.js", library.id),
            ("text/javascript; charset=utf-8", library.bytes.clone()),
        );
    }
    for component in page.components {
        session.resources.insert(
            format!("/{}.js", component.import_path()),
            (
                "text/javascript; charset=utf-8",
                Arc::new(component.source.clone().into_bytes()),
            ),
        );
    }
    for (family, weight, path) in page.fonts {
        if let Ok(bytes) = fs::read(path) {
            session.resources.insert(
                format!("/fonts/{family}-{weight}"),
                ("font/ttf", Arc::new(bytes)),
            );
        }
    }
    let target = session.call(
        "Target.createTarget",
        json!({"url": "about:blank"}),
        READY_TIMEOUT,
    )?;
    let attached = session.call(
        "Target.attachToTarget",
        json!({"targetId": target["targetId"], "flatten": true}),
        READY_TIMEOUT,
    )?;
    session.session = attached["sessionId"].as_str().map(str::to_owned);
    for (method, params) in [
        ("Fetch.enable", json!({"patterns": [{"urlPattern": "*"}]})),
        ("Runtime.enable", json!({})),
        ("Page.enable", json!({})),
        (
            "Emulation.setDeviceMetricsOverride",
            json!({"width": size.0, "height": size.1, "deviceScaleFactor": 1, "mobile": false}),
        ),
        (
            "Emulation.setDefaultBackgroundColorOverride",
            json!({"color": {"r": 0, "g": 0, "b": 0, "a": 0}}),
        ),
        (
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source": CLOCK}),
        ),
        (
            "Page.navigate",
            json!({"url": format!("{ORIGIN}/page.html")}),
        ),
    ] {
        session.call(method, params, READY_TIMEOUT)?;
    }
    let drawable = session.evaluate("window.__arrumoReady()", READY_TIMEOUT)?;
    if let Some(error) = session.errors.first() {
        return Err(io::Error::other(format!("The canvas page failed: {error}")));
    }
    if drawable != Value::Bool(true) {
        return Err(io::Error::other(
            "The canvas page defines no window.render(t) and has no animations to play",
        ));
    }
    fs::create_dir_all(output)?;
    for frame in 0..frames {
        let t = f64::from(frame) / f64::from(fps);
        session.evaluate(&format!("window.__arrumoStep({t})"), FRAME_TIMEOUT)?;
        if let Some(error) = session.errors.first() {
            return Err(io::Error::other(format!("The canvas page failed: {error}")));
        }
        let shot = session.call(
            "Page.captureScreenshot",
            json!({"format": "png", "fromSurface": true, "optimizeForSpeed": true}),
            FRAME_TIMEOUT,
        )?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(shot["data"].as_str().unwrap_or_default())
            .map_err(io::Error::other)?;
        fs::write(output.join(format!("f{frame:05}.png")), png)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_runtime() -> Option<CanvasRuntime> {
    let browser = PathBuf::from(std::env::var_os("ARRUMO_TEST_CANVAS_BROWSER")?);
    // Layers are encoded with FFmpeg, which some test machines lack.
    let encodes = crate::quiet_command(crate::media_tools::ffmpeg())
        .arg("-version")
        .output()
        .is_ok();
    (encodes && browser.is_file()).then(|| CanvasRuntime {
        browser,
        libraries: vec![Library {
            id: "tween".into(),
            module: true,
            description: "Easing".into(),
            bytes: Arc::new(
                b"export const ease = (t) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2);"
                    .to_vec(),
            ),
        }],
        guide: "Draw in window.render(t).".into(),
        bubblewrap: bubblewrap(),
    })
}

/// Lets this test thread draw canvas layers with the browser `ARRUMO_TEST_CANVAS_BROWSER` names.
#[cfg(test)]
pub(crate) fn use_test_runtime() -> bool {
    let runtime = test_runtime().map(Arc::new);
    let ready = runtime.is_some();
    TEST_ACTIVE.with(|active| *active.borrow_mut() = runtime);
    ready
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(path: &Path, x: u32, y: u32) -> [u8; 4] {
        let raw = Command::new(crate::media_tools::ffmpeg())
            .args(["-v", "error", "-i"])
            .arg(path)
            .args(["-vf"])
            .arg(format!("crop=1:1:{x}:{y}"))
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .output()
            .unwrap()
            .stdout;
        raw[..4].try_into().unwrap()
    }

    #[test]
    fn draws_frames_on_a_clock_without_network() {
        let Some(runtime) = test_runtime() else {
            eprintln!(
                "skipped: set ARRUMO_TEST_CANVAS_BROWSER to a headless browser and install FFmpeg"
            );
            return;
        };
        let output = tempfile::tempdir().unwrap();
        let html = r#"<canvas id="c" width="64" height="32"></canvas>
<script type="module">
import { ease } from "tween";
const leak = fetch("https://example.com/").then(() => "reached", () => "blocked");
window.arrumoReady = leak.then((result) => { window.network = result; });
const c = document.getElementById("c").getContext("2d");
window.render = (t) => {
  c.clearRect(0, 0, 64, 32);
  if (window.network !== "blocked") return;
  c.fillStyle = arrumo.values.color;
  c.fillRect(0, 0, Math.round(64 * ease(Math.min(t, 1))), 32);
};
</script>"#;
        let mut values = serde_json::Map::new();
        values.insert("color".into(), json!("#ff0000"));
        let page = CanvasPage {
            html,
            libraries: &["tween".into()],
            values: &values,
            fonts: &[],
            theme: &json!({"colors": ["#123456", "red", "#abc"], "accent": "#00ff00", "text": "x;}"}),
            components: &[],
        };
        let document = page_document(&page, &runtime, (64, 32), 10, 1.0);
        assert!(
            document.contains(
                "--arrumo-accent:#00ff00;--arrumo-color-1:#123456;--arrumo-color-2:#abc;"
            )
        );
        assert!(!document.contains("x;}"), "malformed colours are left out");
        render_frames(&runtime, &page, (64, 32), 10, 11, output.path()).unwrap();
        let first = output.path().join("f00000.png");
        let middle = output.path().join("f00005.png");
        let last = output.path().join("f00010.png");
        assert_eq!(
            pixel(&first, 2, 16)[3],
            0,
            "nothing drawn yet is transparent"
        );
        assert_eq!(pixel(&middle, 2, 16), [255, 0, 0, 255]);
        assert_eq!(
            pixel(&middle, 60, 16)[3],
            0,
            "half way, the bar covers half"
        );
        assert_eq!(pixel(&last, 60, 16), [255, 0, 0, 255]);

        let broken = CanvasPage {
            html: "<script>window.render = () => { throw new Error('boom'); };</script>",
            libraries: &[],
            values: &values,
            fonts: &[],
            theme: &Value::Null,
            components: &[],
        };
        let error = render_frames(&runtime, &broken, (16, 16), 10, 2, &output.path().join("b"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("boom"), "{error}");
    }

    #[test]
    fn pages_import_workspace_components_with_their_libraries() {
        let Some(runtime) = test_runtime() else {
            eprintln!(
                "skipped: set ARRUMO_TEST_CANVAS_BROWSER to a headless browser and install FFmpeg"
            );
            return;
        };
        let output = tempfile::tempdir().unwrap();
        // The component needs the tween library although the page doesn't list it.
        let component = crate::canvas_components::CanvasComponent {
            slug: "fill".into(),
            source: r#"import { ease } from "tween";
export default function (element, options = {}) {
  const canvas = document.createElement("canvas");
  canvas.width = arrumo.width; canvas.height = arrumo.height;
  element.appendChild(canvas);
  const c = canvas.getContext("2d");
  return (t) => {
    c.clearRect(0, 0, canvas.width, canvas.height);
    c.fillStyle = options.color ?? arrumo.theme.accent;
    c.fillRect(0, 0, Math.round(canvas.width * ease(Math.min(t, 1))), canvas.height);
  };
}"#
            .into(),
            options: vec![],
            libraries: vec!["tween".into()],
        };
        let page = CanvasPage {
            html: r#"<script type="module">
import fill from "workspace/fill";
window.render = fill(document.body);
</script>"#,
            libraries: &[],
            values: &serde_json::Map::new(),
            fonts: &[],
            theme: &json!({"accent": "#0000ff"}),
            components: std::slice::from_ref(&component),
        };
        render_frames(&runtime, &page, (32, 16), 10, 11, output.path()).unwrap();
        assert_eq!(
            pixel(&output.path().join("f00010.png"), 30, 8),
            [0, 0, 255, 255],
            "the component draws in the theme's accent"
        );
    }

    #[test]
    fn descriptors_need_safe_paths_and_downloads() {
        let canvas: CanvasRendererDescriptor = serde_json::from_value(json!({
            "browser": {"linux-x64": {"url": "https://example.com/b.zip", "sha256": "a".repeat(64), "size": 10, "executable": "b/shell"}},
            "libraries": [{"id": "three", "version": "0.170.0", "file": "lib/three.js", "module": true, "license": "MIT", "description": "3D"}],
            "guide": "guide.md"
        }))
        .unwrap();
        assert!(canvas.valid());
        for change in [
            json!({"guide": "../guide.md"}),
            json!({"browser": {"amiga": canvas.browser["linux-x64"]}}),
            json!({"browser": {"linux-x64": {"url": "http://example.com/b.zip", "sha256": "a".repeat(64), "size": 10, "executable": "b"}}}),
            json!({"browser": {"linux-x64": {"url": "https://example.com/b.zip", "sha256": "a".repeat(64), "size": 10, "executable": "../b"}}}),
            json!({"libraries": [canvas.libraries[0], canvas.libraries[0]]}),
        ] {
            let mut value = serde_json::to_value(&canvas).unwrap();
            for (key, field) in change.as_object().unwrap() {
                value[key] = field.clone();
            }
            let broken: CanvasRendererDescriptor = serde_json::from_value(value).unwrap();
            assert!(!broken.valid(), "{change}");
        }
    }
}
