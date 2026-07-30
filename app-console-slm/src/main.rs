// app-console-slm: Sovereign operator console for Foundry AI infrastructure.
// Sprint 4a: `status` subcommand — live Doorman + corpus snapshot.
// Sprint 4b (deferred): `watch` (repeat), `admin` subcommands.

use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "app-console-slm", about = "Foundry AI infrastructure console")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show live status of Doorman, tiers, chassis, and corpus.
    Status {
        /// Doorman base URL.
        #[arg(
            long,
            env = "SLM_DOORMAN_ENDPOINT",
            default_value = "http://127.0.0.1:9080"
        )]
        doorman: String,
        /// Chassis base URL.
        #[arg(
            long,
            env = "SLM_ORCHESTRATION_ENDPOINT",
            default_value = "http://127.0.0.1:9180"
        )]
        chassis: String,
        /// Corpus data directory (contains queue/, queue-done/, queue-poison/ subdirs).
        #[arg(
            long,
            env = "CORPUS_ROOT",
            default_value = "/srv/foundry/data/apprenticeship"
        )]
        corpus: String,
    },
}

// ── Response structs ─────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct HealthzResponse {
    #[serde(default)]
    entity_count: Option<u64>,
    // chassis /healthz uses "status": "ok"
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize, Default)]
struct ReadyzResponse {
    #[serde(default)]
    node_class: Option<String>,
    #[serde(default)]
    tier_a: bool,
    #[serde(default)]
    has_local: bool,
    #[serde(default)]
    tier_b: HashMap<String, TierBInfo>,
}

#[derive(Deserialize, Default)]
struct TierBInfo {
    #[serde(default)]
    health_up: bool,
    #[serde(default)]
    circuit: String,
    #[serde(default)]
    opened_for_secs: Option<u64>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    zone: Option<String>,
}

#[derive(Deserialize, Default)]
struct ChassisReadyzResponse {
    #[serde(default)]
    fleet_members: Option<u32>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default()
}

fn get_json<T: for<'de> Deserialize<'de> + Default>(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Option<T> {
    client.get(url).send().ok()?.json::<T>().ok()
}

fn count_dir(path: &Path) -> usize {
    std::fs::read_dir(path)
        .map(|entries| entries.filter(|e| e.is_ok()).count())
        .unwrap_or(0)
}

pub fn format_duration(secs: u64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    let rem_mins = mins % 60;
    if hours < 24 {
        return if rem_mins == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h {rem_mins}m")
        };
    }
    let days = hours / 24;
    let rem_hours = hours % 24;
    if rem_hours == 0 {
        format!("{days}d")
    } else {
        format!("{days}d {rem_hours}h")
    }
}

fn col_width() -> (usize, usize, usize) {
    (12, 26, 0)
}

fn up_down(up: bool) -> &'static str {
    if up {
        "UP  "
    } else {
        "DOWN"
    }
}

// ── Status command ────────────────────────────────────────────────────────────

fn cmd_status(doorman_url: &str, chassis_url: &str, corpus_root: &str) {
    let client = http_client();
    let (label_w, url_w, _) = col_width();

    // ── Doorman /healthz + /readyz ───────────────────────────────────────────
    // /healthz returns 404 in current builds (known bug — route not yet added).
    // Fall back to /readyz to determine Doorman availability.
    let healthz: Option<HealthzResponse> = get_json(&client, &format!("{doorman_url}/healthz"));
    let readyz: Option<ReadyzResponse> = get_json(&client, &format!("{doorman_url}/readyz"));
    let doorman_up = healthz.is_some() || readyz.is_some();
    let entity_count = healthz.as_ref().and_then(|h| h.entity_count).unwrap_or(0);

    println!(
        "{:<label_w$} {:<url_w$} {}  entity_count={entity_count}",
        "Doorman",
        doorman_url,
        up_down(doorman_up),
    );

    if let Some(ref rz) = readyz {
        let node_class = rz.node_class.as_deref().unwrap_or("unknown");
        let tier_a_detail = if rz.has_local {
            format!("OLMo 7B Instruct Q4_K_M  (node_class={node_class})")
        } else {
            "not available".to_string()
        };
        println!(
            "{:<label_w$} {:<url_w$} {}  {}",
            "Tier A",
            "",
            up_down(rz.tier_a),
            tier_a_detail,
        );

        if rz.tier_b.is_empty() {
            println!(
                "{:<label_w$} {:<url_w$} {}  not configured",
                "Tier B",
                "",
                up_down(false)
            );
        } else {
            for (label, info) in &rz.tier_b {
                let mut detail = format!("circuit={}", info.circuit);
                if let Some(ref reason) = info.reason {
                    detail.push_str(&format!("  reason={reason}"));
                }
                if let Some(ref zone) = info.zone {
                    detail.push_str(&format!("  zone={zone}"));
                }
                if let Some(secs) = info.opened_for_secs {
                    detail.push_str(&format!("  ({})", format_duration(secs)));
                }
                let tier_label = format!("Tier B [{label}]");
                println!(
                    "{:<label_w$} {:<url_w$} {}  {}",
                    tier_label,
                    "",
                    up_down(info.health_up),
                    detail,
                );
            }
        }
    } else {
        println!(
            "{:<label_w$} {:<url_w$} {}  /readyz unavailable",
            "Tier A",
            "",
            up_down(false)
        );
        println!(
            "{:<label_w$} {:<url_w$} {}  /readyz unavailable",
            "Tier B",
            "",
            up_down(false)
        );
    }

    // ── Chassis /healthz ─────────────────────────────────────────────────────
    let chassis_health: Option<HealthzResponse> =
        get_json(&client, &format!("{chassis_url}/healthz"));
    let chassis_up = chassis_health
        .as_ref()
        .map(|h| h.status.as_deref() == Some("ok"))
        .unwrap_or(false);

    let chassis_detail = if chassis_up {
        let chassis_readyz: Option<ChassisReadyzResponse> =
            get_json(&client, &format!("{chassis_url}/readyz"));
        chassis_readyz
            .and_then(|r| r.fleet_members)
            .map(|n| format!("fleet={n} member(s)"))
            .unwrap_or_else(|| "fleet=unknown".to_string())
    } else {
        "not deployed".to_string()
    };
    println!(
        "{:<label_w$} {:<url_w$} {}  {chassis_detail}",
        "Chassis",
        chassis_url,
        up_down(chassis_up),
    );

    // ── Corpus counts ─────────────────────────────────────────────────────────
    let root = Path::new(corpus_root);
    let pending = count_dir(&root.join("queue"));
    let done = count_dir(&root.join("queue-done"));
    let poison = count_dir(&root.join("queue-poison"));

    // Approximate SFT / DPO split: SFT = queue-done entries with no actual_diff or non-empty diff
    // (all done entries are SFT from the git post-commit hook at this stage of development).
    // DPO count remains low until Yo-Yo is running; show raw counts until corpus-threshold
    // script runs.
    println!(
        "{:<label_w$} {:<url_w$}       queue={pending}  done={done}  poison={poison}",
        "Corpus", corpus_root,
    );
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status {
            doorman,
            chassis,
            corpus,
        } => {
            cmd_status(&doorman, &chassis, &corpus);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(59), "59s");
    }

    #[test]
    fn format_duration_minutes() {
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(90), "1m");
        assert_eq!(format_duration(3599), "59m");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(3660), "1h 1m");
        assert_eq!(format_duration(5400), "1h 30m");
    }

    #[test]
    fn format_duration_days() {
        assert_eq!(format_duration(86400), "1d");
        assert_eq!(format_duration(90000), "1d 1h");
    }

    #[test]
    fn count_dir_returns_file_count() {
        let dir = TempDir::new().unwrap();
        for i in 0..5_u8 {
            let mut f = std::fs::File::create(dir.path().join(format!("{i}.jsonl"))).unwrap();
            f.write_all(b"{}").unwrap();
        }
        assert_eq!(count_dir(dir.path()), 5);
    }

    #[test]
    fn count_dir_missing_returns_zero() {
        assert_eq!(count_dir(Path::new("/nonexistent/path")), 0);
    }
}
