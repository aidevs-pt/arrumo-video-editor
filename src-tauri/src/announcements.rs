//! Messages from the Arrumo team, such as "update this version by hand". The list lives in
//! `announcements/announcements.json` on the repository's main branch, signed as a whole with the
//! updater key by the Announcements workflow, so only release maintainers can add or remove one.
//!
//! At launch Arrumo fetches the list and its signature and verifies the exact bytes against the
//! public key built into the app. It never accepts a list with a lower `serial` than one it has
//! already seen, so an older, signed list can't bring back a removed message. It shows at most one
//! message per launch, the first that applies to this version and wasn't closed before.

use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const LIST_URL: &str = "https://raw.githubusercontent.com/aidevs-pt/arrumo-video-editor/main/announcements/announcements.json";
const TIMEOUT: Duration = Duration::from_secs(8);
/// Larger lists are refused rather than parsed.
const MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
struct Board {
    serial: u64,
    #[serde(default)]
    messages: Vec<Message>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Message {
    pub id: String,
    /// The message by language code (`en`, `es`, `pt`); English is required.
    pub text: BTreeMap<String, String>,
    #[serde(default)]
    pub link: Option<String>,
    /// The app versions it applies to: from `from` (inclusive) up to `before` (exclusive).
    #[serde(default, skip_serializing)]
    pub versions: Versions,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Versions {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    before: Option<String>,
}

/// What this computer has seen: the highest serial and the messages that were closed.
#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
struct Seen {
    serial: u64,
    dismissed: Vec<String>,
}

/// Whether the bytes are exactly what the holder of the updater key signed.
fn verify(bytes: &[u8], signature: &str, public_key: &str) -> bool {
    let decode = |value: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(value.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    };
    let (Some(key), Some(signature)) = (decode(public_key), decode(signature)) else {
        return false;
    };
    let (Ok(key), Ok(signature)) = (PublicKey::decode(&key), Signature::decode(&signature)) else {
        return false;
    };
    key.verify(bytes, &signature, true).is_ok()
}

fn applies(versions: &Versions, current: &semver::Version) -> bool {
    let parse = |value: &Option<String>| value.as_deref().map(semver::Version::parse);
    match (parse(&versions.from), parse(&versions.before)) {
        (Some(Err(_)), _) | (_, Some(Err(_))) => false,
        (from, before) => {
            from.is_none_or(|from| from.is_ok_and(|from| *current >= from))
                && before.is_none_or(|before| before.is_ok_and(|before| *current < before))
        }
    }
}

/// The message to show from a verified list, and the serial to remember. `None` when the list
/// is older than one already seen, or nothing in it applies.
fn pick(bytes: &[u8], seen: &Seen, current: &semver::Version) -> Option<(Message, u64)> {
    let board: Board = serde_json::from_slice(bytes).ok()?;
    if board.serial < seen.serial {
        return None;
    }
    let message = board.messages.into_iter().find(|message| {
        !message.id.is_empty()
            && !seen.dismissed.contains(&message.id)
            && message
                .text
                .get("en")
                .is_some_and(|text| !text.trim().is_empty())
            && message
                .link
                .as_deref()
                .is_none_or(|link| link.starts_with("https://"))
            && applies(&message.versions, current)
    })?;
    Some((message, board.serial))
}

fn seen_path(app: &AppHandle) -> Option<PathBuf> {
    Some(
        app.path()
            .app_local_data_dir()
            .ok()?
            .join("announcements.json"),
    )
}

fn read_seen(app: &AppHandle) -> Seen {
    seen_path(app)
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_seen(app: &AppHandle, seen: &Seen) {
    if let (Some(path), Ok(bytes)) = (seen_path(app), serde_json::to_vec(seen)) {
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        let _ = fs::write(path, bytes);
    }
}

fn download(url: &str) -> Option<Vec<u8>> {
    let response = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .ok()?
        .get(url)
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    let bytes = response.bytes().ok()?;
    (bytes.len() <= MAX_BYTES).then(|| bytes.to_vec())
}

/// The message to show at this launch, if any. Any failure (offline, unsigned build, a list that
/// doesn't verify) shows nothing.
#[tauri::command]
pub async fn pending_announcement(app: AppHandle) -> Option<Message> {
    let public_key = crate::updates::public_key()?;
    let current = app.package_info().version.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Debug builds can read a local list, to try a message before publishing it.
        #[cfg(debug_assertions)]
        let url = std::env::var("ARRUMO_ANNOUNCEMENTS_URL").unwrap_or_else(|_| LIST_URL.into());
        #[cfg(not(debug_assertions))]
        let url = LIST_URL.to_owned();
        let bytes = download(&url)?;
        let signature = String::from_utf8(download(&format!("{url}.sig"))?).ok()?;
        if !verify(&bytes, &signature, public_key) {
            return None;
        }
        let mut seen = read_seen(&app);
        let (message, serial) = pick(&bytes, &seen, &current)?;
        if serial > seen.serial {
            seen.serial = serial;
            write_seen(&app, &seen);
        }
        Some(message)
    })
    .await
    .ok()
    .flatten()
}

/// Closes a message for good on this computer.
#[tauri::command]
pub fn dismiss_announcement(app: AppHandle, id: String) {
    let mut seen = read_seen(&app);
    if !seen.dismissed.contains(&id) {
        seen.dismissed.push(id);
        write_seen(&app, &seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Signed with a throwaway key by `tauri signer sign`, exactly as the workflow does.
    const PUBLIC_KEY: &str = include_str!("../tests/announcements/key.pub");
    const LIST: &[u8] = include_bytes!("../tests/announcements/announcements.json");
    const SIGNATURE: &str = include_str!("../tests/announcements/announcements.json.sig");
    const OTHER_KEY: &str = include_str!("../tests/announcements/other-key.pub");

    fn version(value: &str) -> semver::Version {
        semver::Version::parse(value).unwrap()
    }

    #[test]
    fn a_list_from_the_workflow_verifies_and_shows_its_message() {
        assert!(verify(LIST, SIGNATURE, PUBLIC_KEY));
        let (message, serial) = pick(LIST, &Seen::default(), &version("0.1.0")).unwrap();
        assert_eq!((message.id.as_str(), serial), ("2026-12-update-by-hand", 3));
        assert_eq!(pick(LIST, &Seen::default(), &version("0.1.1")), None);
    }

    #[test]
    fn only_the_exact_signed_bytes_verify() {
        assert!(verify(LIST, SIGNATURE, PUBLIC_KEY));
        let mut changed = LIST.to_vec();
        changed[10] ^= 1;
        assert!(!verify(&changed, SIGNATURE, PUBLIC_KEY));
        assert!(!verify(LIST, SIGNATURE, OTHER_KEY));
        assert!(!verify(LIST, "not a signature", PUBLIC_KEY));
    }

    #[test]
    fn the_first_message_for_this_version_that_was_not_closed_is_shown() {
        let list = br#"{"serial": 4, "messages": [
            {"id": "old", "text": {"en": "Only before 0.2"}, "versions": {"before": "0.2.0"}},
            {"id": "insecure", "text": {"en": "Bad link"}, "link": "http://example.com"},
            {"id": "closed", "text": {"en": "Closed before"}},
            {"id": "now", "text": {"en": "Update by hand", "pt": "Atualiza"}, "link": "https://aidevs.pt",
             "versions": {"from": "0.2.0", "before": "0.3.0"}},
            {"id": "later", "text": {"en": "Second"}}
        ]}"#;
        let seen = Seen {
            serial: 3,
            dismissed: vec!["closed".into()],
        };
        let (message, serial) = pick(list, &seen, &version("0.2.5")).unwrap();
        assert_eq!((message.id.as_str(), serial), ("now", 4));
        assert_eq!(message.text.get("pt").map(String::as_str), Some("Atualiza"));
        assert_eq!(pick(list, &seen, &version("0.1.0")).unwrap().0.id, "old");
        assert_eq!(pick(list, &seen, &version("0.3.0")).unwrap().0.id, "later");
    }

    #[test]
    fn an_older_list_is_refused() {
        let list = br#"{"serial": 2, "messages": [{"id": "withdrawn", "text": {"en": "Gone"}}]}"#;
        let seen = Seen {
            serial: 3,
            dismissed: vec![],
        };
        assert_eq!(pick(list, &seen, &version("0.1.0")), None);
    }

    #[test]
    fn messages_without_english_or_with_bad_versions_are_skipped() {
        let list = br#"{"serial": 1, "messages": [
            {"id": "no-english", "text": {"pt": "Ola"}},
            {"id": "bad-version", "text": {"en": "Hi"}, "versions": {"from": "soon"}}
        ]}"#;
        assert_eq!(pick(list, &Seen::default(), &version("0.1.0")), None);
    }
}
