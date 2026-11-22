//! Reads how much of each provider's allowance is used, through the plugins' SDK `usage.json`
//! descriptors, so the app can warn before a provider runs out.

use rusqlite::OptionalExtension;
use std::{error::Error, io, io::Read, time::Duration};

use reqwest::header::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    PluginSettingKind, WorkspaceStore,
    plugin_catalog::{BUNDLED, bundled_manifests},
};

const MAX_RESPONSE_BYTES: u64 = 256 * 1024;
/// At or below this share left, the app warns.
pub const USAGE_WARNING_PERCENT: f64 = 20.0;
/// At or below this share left, the warning uses the danger color.
pub const USAGE_DANGER_PERCENT: f64 = 10.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UsageAdapter {
    schema_version: u32,
    plugin_id: String,
    endpoint: String,
    credential_setting: String,
    credential_header: String,
    credential_prefix: String,
    used: String,
    limit: String,
    #[serde(default)]
    resets_at: Option<String>,
    unit: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub plugin_id: String,
    pub unit: String,
    /// `None` when the provider could not be read; the reason is in `problem`.
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub resets_at: Option<i64>,
    /// For `percent` usage: the length of the allowance window in minutes, such as a week.
    pub window_minutes: Option<i64>,
    /// `ok`, `warning` (20% or less left), `danger` (10% or less) or `unknown`.
    pub level: &'static str,
    pub problem: Option<String>,
}

fn pointer(value: &str) -> bool {
    value.starts_with('/') && value.len() <= 128
}

fn usage_adapter(index: usize) -> Option<UsageAdapter> {
    let manifest = bundled_manifests().get(index)?;
    let adapter: UsageAdapter = serde_json::from_slice(BUNDLED.get(index)?.usage?).ok()?;
    let has_secret = manifest.settings.iter().flatten().any(|setting| {
        setting.id == adapter.credential_setting && setting.kind == PluginSettingKind::Secret
    });
    let host_ok = adapter
        .endpoint
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| !host.is_empty())
        && !adapter.endpoint.contains(['#', '@', ' ']);
    (adapter.schema_version == 1
        && adapter.plugin_id == manifest.id
        && has_secret
        && host_ok
        && matches!(adapter.credential_prefix.as_str(), "" | "Bearer " | "Key ")
        && matches!(
            adapter.unit.as_str(),
            "characters" | "credits" | "requests" | "seconds"
        )
        && pointer(&adapter.used)
        && pointer(&adapter.limit)
        && adapter.resets_at.as_deref().is_none_or(pointer))
    .then_some(adapter)
}

/// The level for a share left, in percent.
pub fn usage_level(remaining_percent: f64) -> &'static str {
    if remaining_percent <= USAGE_DANGER_PERCENT {
        "danger"
    } else if remaining_percent <= USAGE_WARNING_PERCENT {
        "warning"
    } else {
        "ok"
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|number: &f64| number.is_finite() && *number >= 0.0)
}

fn read_usage(adapter: &UsageAdapter, secret: &[u8]) -> io::Result<(f64, f64, Option<i64>)> {
    let header = HeaderName::from_bytes(adapter.credential_header.as_bytes())
        .map_err(|_| io::Error::other("Invalid usage adapter"))?;
    if matches!(
        header.as_str(),
        "host" | "content-type" | "content-length" | "transfer-encoding" | "connection"
    ) {
        return Err(io::Error::other("Invalid usage adapter"));
    }
    let mut bytes = zeroize::Zeroizing::new(adapter.credential_prefix.as_bytes().to_vec());
    bytes.extend_from_slice(secret);
    let mut value = HeaderValue::from_bytes(&bytes)
        .map_err(|_| io::Error::other("Invalid credential format"))?;
    value.set_sensitive(true);
    let response = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| io::Error::other("Could not initialize usage transport"))?
        .get(&adapter.endpoint)
        .header(header, value)
        .send()
        .map_err(|_| io::Error::other("The provider could not be reached"))?;
    if !response.status().is_success() {
        let code = response.status().as_u16();
        let mut body = Vec::new();
        let _ = response.take(4096).read_to_end(&mut body);
        let reason = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|body| {
                ["/detail/status", "/detail", "/error", "/message"]
                    .iter()
                    .find_map(|at| body.pointer(at)?.as_str().map(str::to_owned))
            })
            .map(|reason| {
                format!(
                    ": {}",
                    reason
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(120)
                        .collect::<String>()
                )
            })
            .unwrap_or_default();
        return Err(io::Error::other(format!(
            "The provider answered HTTP {code}{reason}"
        )));
    }
    let mut text = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(|_| io::Error::other("Could not read the provider's answer"))?;
    if text.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(io::Error::other("The provider's answer is too large"));
    }
    let json: Value = serde_json::from_slice(&text)
        .map_err(|_| io::Error::other("The provider's answer is not JSON"))?;
    let used = number(json.pointer(&adapter.used))
        .ok_or_else(|| io::Error::other("The provider did not report usage"))?;
    let limit = number(json.pointer(&adapter.limit))
        .filter(|limit| *limit > 0.0)
        .ok_or_else(|| io::Error::other("The provider did not report a limit"))?;
    let resets_at = adapter
        .resets_at
        .as_deref()
        .and_then(|at| json.pointer(at)?.as_i64());
    Ok((used, limit, resets_at))
}

fn summarize(
    plugin_id: &str,
    unit: &str,
    read: io::Result<(f64, f64, Option<i64>)>,
) -> ProviderUsage {
    match read {
        Ok((used, limit, resets_at)) => {
            let remaining = ((limit - used.min(limit)) / limit * 100.0).clamp(0.0, 100.0);
            ProviderUsage {
                plugin_id: plugin_id.to_owned(),
                unit: unit.to_owned(),
                used: Some(used),
                limit: Some(limit),
                remaining_percent: Some((remaining * 10.0).round() / 10.0),
                resets_at,
                window_minutes: None,
                level: usage_level(remaining),
                problem: None,
            }
        }
        Err(error) => ProviderUsage {
            plugin_id: plugin_id.to_owned(),
            unit: unit.to_owned(),
            used: None,
            limit: None,
            remaining_percent: None,
            resets_at: None,
            window_minutes: None,
            level: "unknown",
            problem: Some(error.to_string()),
        },
    }
}

impl WorkspaceStore {
    /// The allowance left on each installed provider that describes its usage and has a key for
    /// this workspace. Providers are read one after another; one that fails is `unknown`.
    pub fn provider_usage(&self, workspace_id: i64) -> Result<Vec<ProviderUsage>, Box<dyn Error>> {
        let installed: Vec<String> = self
            .active_plugin_catalog()?
            .into_iter()
            .map(|plugin| plugin.manifest.id)
            .collect();
        let mut usage = Vec::new();
        for (index, manifest) in bundled_manifests().iter().enumerate() {
            if !installed.contains(&manifest.id) {
                continue;
            }
            let Some(adapter) = usage_adapter(index) else {
                continue;
            };
            let Some(secret) =
                self.provider_secret(&manifest.id, &adapter.credential_setting, workspace_id)?
            else {
                continue;
            };
            usage.push(summarize(
                &manifest.id,
                &adapter.unit,
                read_usage(&adapter, &secret),
            ));
        }
        for manifest in bundled_manifests() {
            let Some(plugin) = installed
                .contains(&manifest.id)
                .then(|| crate::agent_plugin(&manifest.id))
                .flatten()
            else {
                continue;
            };
            let read = match crate::agent_cli::read_agent_usage(plugin) {
                Some(read) => read,
                None if plugin.cli.usage_event.is_some() => {
                    // Known only from the agent's last run, until that window resets.
                    let Some(seen) = self
                        .0
                        .query_row(
                            "SELECT used_percent, resets_at, window_minutes
                             FROM agent_usage_observations
                             WHERE plugin_id = ?1 AND (resets_at IS NULL OR resets_at > unixepoch())
                             ORDER BY used_percent DESC, window_minutes DESC LIMIT 1",
                            [&manifest.id],
                            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                        )
                        .optional()?
                    else {
                        continue;
                    };
                    Ok(seen)
                }
                None => continue,
            };
            usage.push(match read {
                Ok((used, resets_at, window_minutes)) => ProviderUsage {
                    window_minutes,
                    ..summarize(&manifest.id, "percent", Ok((used, 100.0, resets_at)))
                },
                Err(error) => summarize(&manifest.id, "percent", Err(error)),
            });
        }
        Ok(usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_warning_thresholds() {
        assert_eq!(usage_level(55.0), "ok");
        assert_eq!(usage_level(20.0), "warning");
        assert_eq!(usage_level(12.0), "warning");
        assert_eq!(usage_level(10.0), "danger");
        assert_eq!(usage_level(0.0), "danger");
    }

    #[test]
    fn summaries_round_and_clamp() {
        let usage = summarize(
            "elevenlabs",
            "characters",
            Ok((8_800.0, 10_000.0, Some(1_738_356_858))),
        );
        assert_eq!(usage.remaining_percent, Some(12.0));
        assert_eq!(usage.level, "warning");
        let over = summarize("elevenlabs", "characters", Ok((12_000.0, 10_000.0, None)));
        assert_eq!((over.remaining_percent, over.level), (Some(0.0), "danger"));
        let failed = summarize(
            "elevenlabs",
            "characters",
            Err(io::Error::other("The provider answered HTTP 401")),
        );
        assert_eq!((failed.level, failed.remaining_percent), ("unknown", None));
    }

    #[test]
    fn numbers_may_arrive_as_strings_but_not_negative() {
        assert_eq!(number(Some(&serde_json::json!("42"))), Some(42.0));
        assert_eq!(number(Some(&serde_json::json!(-1))), None);
        assert_eq!(number(Some(&serde_json::json!("unlimited"))), None);
    }

    #[test]
    fn claude_code_usage_comes_from_its_rate_limit_events() {
        let event = crate::agent_plugin("claude-code")
            .and_then(|plugin| plugin.cli.usage_event.as_ref())
            .expect("Claude Code reports usage while it runs");
        let output = concat!(
            "{\"type\":\"assistant\"}\n",
            "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{\"unifiedWindows\":{",
            "\"five_hour\":{\"utilization\":0.45,\"resetsAt\":1795384200},",
            "\"seven_day\":{\"utilization\":0.13,\"resetsAt\":1795662000}}}}\n",
            "{\"type\":\"result\"}\n",
        );
        assert_eq!(
            event.read(output),
            Some(vec![
                (0, (45.0, Some(1_795_384_200), Some(300))),
                (1, (13.0, Some(1_795_662_000), Some(10_080))),
            ])
        );
        assert_eq!(event.read("{\"type\":\"result\"}"), None);
    }

    #[test]
    fn codex_describes_a_usage_read_with_reset_times() {
        let run = crate::agent_plugin("codex")
            .and_then(|plugin| plugin.cli.usage_run.as_ref())
            .expect("Codex reads its plan usage");
        assert_eq!(run.args, ["app-server"]);
        assert!(run.windows.iter().all(|window| window.resets_at.is_some()));
    }

    #[test]
    fn the_bundled_elevenlabs_usage_adapter_is_valid() {
        let index = bundled_manifests()
            .iter()
            .position(|manifest| manifest.id == "elevenlabs")
            .unwrap();
        assert_eq!(
            usage_adapter(index).expect("valid usage adapter").unit,
            "characters"
        );
    }
}
