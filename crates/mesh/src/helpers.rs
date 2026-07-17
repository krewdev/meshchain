//! Shared helper functions used across command modules.

use anyhow::{bail, Context, Result};
use meshchain_ledger::genesis::ONE_MESH;
use meshchain_ledger::state::ChainState;
use meshchain_proto::crypto::Keypair;
use meshchain_proto::pq::PqKeypair;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

// ── Path helpers ──────────────────────────────────────────────────────────────

pub fn keys_dir(dir: &Path) -> PathBuf {
    dir.join("keys")
}

pub fn wallet_path(dir: &Path, name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() || name.contains('/') {
        p
    } else {
        keys_dir(dir).join(name)
    }
}

/// Prefer the freshest chain_state among lab snapshot and live v0 validator tree.
pub fn best_chain_state_path(dir: &Path) -> PathBuf {
    let candidates = [
        dir.join("chain_state.json"),
        dir.join("v0").join("chain_state.json"),
    ];
    let mut best = candidates[0].clone();
    let mut best_h: i64 = -1;
    for p in &candidates {
        if let Ok(st) = ChainState::load_json(p) {
            let h = st.height as i64;
            if h >= best_h {
                best_h = h;
                best = p.clone();
            }
        }
    }
    best
}

pub fn promote_v0_snapshot(dir: &Path) {
    let v0 = dir.join("v0").join("chain_state.json");
    let snap = dir.join("chain_state.json");
    if v0.exists() {
        if let Err(e) = fs::copy(&v0, &snap) {
            eprintln!("note: could not promote v0 chain_state: {e}");
        }
    }
}

// ── Network defaults ──────────────────────────────────────────────────────────

pub fn is_local_peer(peer: &str) -> bool {
    let host = peer.split(':').next().unwrap_or(peer);
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// Default submit peer: MESH_SUBMIT env → data/seeds.json → repo testnet/seeds.json → localhost
pub fn default_submit_peer(dir: &Path) -> String {
    if let Ok(p) = std::env::var("MESH_SUBMIT") {
        if !p.is_empty() {
            return p;
        }
    }
    for seeds_path in [
        dir.join("seeds.json"),
        PathBuf::from("testnet/seeds.json"),
        PathBuf::from("testnet/published/seeds.json"),
    ] {
        if let Ok(s) = fs::read_to_string(&seeds_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(arr) = v.get("seeds").and_then(|x| x.as_array()) {
                    for seed in arr {
                        let roles = seed
                            .get("roles")
                            .and_then(|r| r.as_array())
                            .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>())
                            .unwrap_or_default();
                        let prefer = roles.iter().any(|r| *r == "submit" || *r == "seed");
                        if prefer || roles.is_empty() {
                            if let (Some(host), Some(port)) = (
                                seed.get("host").and_then(|h| h.as_str()),
                                seed.get("port").and_then(|p| p.as_u64()),
                            ) {
                                return format!("{host}:{port}");
                            }
                        }
                    }
                }
            }
        }
    }
    "127.0.0.1:9100".into()
}

pub fn default_scanner_url(dir: &Path) -> Option<String> {
    if let Ok(p) = std::env::var("MESH_SCANNER") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    for seeds_path in [
        dir.join("seeds.json"),
        PathBuf::from("testnet/seeds.json"),
        PathBuf::from("testnet/published/seeds.json"),
    ] {
        if let Ok(s) = fs::read_to_string(&seeds_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                // Prefer HTTPS when published
                for key in ["/http/scanner_https", "/http/scanner"] {
                    if let Some(u) = v.pointer(key).and_then(|x| x.as_str()) {
                        if !u.is_empty() {
                            return Some(u.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn default_faucet_url(dir: &Path) -> Option<String> {
    if let Ok(p) = std::env::var("MESH_FAUCET") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    for seeds_path in [
        dir.join("seeds.json"),
        PathBuf::from("testnet/seeds.json"),
        PathBuf::from("testnet/published/seeds.json"),
    ] {
        if let Ok(s) = fs::read_to_string(&seeds_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                for key in ["/http/faucet_https", "/http/faucet"] {
                    if let Some(u) = v.pointer(key).and_then(|x| x.as_str()) {
                        if !u.is_empty() {
                            return Some(u.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

// ── HTTP (curl-based, no extra deps) ─────────────────────────────────────────

/// Minimal HTTP GET without extra crates (std only).
pub fn http_get(url: &str) -> Result<String> {
    // Prefer curl for TLS + portability in operator environments.
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", url])
        .output()
        .context("curl failed — install curl to use join-public/sync-state")?;
    if !out.status.success() {
        bail!(
            "HTTP GET failed ({url}): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8(out.stdout).context("response not utf-8")
}

pub fn http_post_json(url: &str, body: &str) -> Result<String> {
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "60",
            "-H",
            "Content-Type: application/json",
            "-d",
            body,
            url,
        ])
        .output()
        .context("curl failed")?;
    if !out.status.success() {
        bail!(
            "HTTP POST failed ({url}): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8(out.stdout).context("response not utf-8")
}

pub fn sync_state_from_scanner(dir: &Path, scanner_base: &str) -> Result<ChainState> {
    let base = scanner_base.trim_end_matches('/');
    let url = format!("{base}/api/v1/chain_state");
    println!("Syncing chain state from {url} …");
    let body = http_get(&url)?;
    let st: ChainState =
        serde_json::from_str(&body).context("scanner returned invalid chain_state JSON")?;
    fs::create_dir_all(dir)?;
    let path = dir.join("chain_state.json");
    fs::write(&path, serde_json::to_string_pretty(&st)?)?;
    println!(
        "Wrote {} (height {} · {} accounts · {})",
        path.display(),
        st.height,
        st.accounts.len(),
        st.chain_id
    );
    Ok(st)
}

// ── Key loaders ───────────────────────────────────────────────────────────────

pub fn load_wallet(path: &Path) -> Result<Keypair> {
    let file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(path).with_context(|| {
            format!(
                "Could not open wallet file {}. Try: mesh new-wallet",
                path.display()
            )
        })?)?;
    Keypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))
}

pub fn load_cold(path: &Path) -> Result<PqKeypair> {
    let file: meshchain_proto::pq::PqKeypairFile =
        serde_json::from_str(&fs::read_to_string(path).with_context(|| {
            format!(
                "Could not open cold key {}. Try: mesh new-cold-key",
                path.display()
            )
        })?)?;
    PqKeypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))
}

// ── Amount helpers ────────────────────────────────────────────────────────────

pub fn parse_mesh_amount(s: &str) -> Result<u64> {
    let s = s.trim();
    if let Ok(whole) = s.parse::<u64>() {
        return Ok(whole.saturating_mul(ONE_MESH));
    }
    // decimal
    let parts: Vec<_> = s.split('.').collect();
    if parts.len() != 2 {
        bail!("Amount should look like 5 or 1.5 (MESH units)");
    }
    let whole: u64 = parts[0].parse().context("bad amount")?;
    let mut frac = parts[1].to_string();
    if frac.len() > 6 {
        bail!("At most 6 digits after the decimal");
    }
    while frac.len() < 6 {
        frac.push('0');
    }
    let frac_n: u64 = frac.parse().context("bad amount")?;
    Ok(whole.saturating_mul(ONE_MESH).saturating_add(frac_n))
}

pub fn fmt_mesh(units: u64) -> String {
    format!("{:.6}", units as f64 / ONE_MESH as f64)
}

// ── Submit helpers ────────────────────────────────────────────────────────────

pub fn refresh_after_submit(dir: &Path, peer: &str) {
    if is_local_peer(peer) {
        thread::sleep(Duration::from_secs(3));
        promote_v0_snapshot(dir);
        return;
    }
    // Remote seed: wait then pull scanner snapshot when available
    thread::sleep(Duration::from_secs(5));
    if let Some(scanner) = default_scanner_url(dir) {
        if let Err(e) = sync_state_from_scanner(dir, &scanner) {
            eprintln!(
                "note: could not sync from scanner ({e}). Run: mesh sync-state --scanner {scanner}"
            );
        }
    } else {
        eprintln!(
            "note: remote submit done. Sync with: mesh sync-state --scanner http://SEED:8788"
        );
    }
}

pub fn submit_tx_to_peer(tx_path: &Path, peer: &str) -> Result<()> {
    let tx = tx_path.to_str().context("tx path is not valid UTF-8")?;
    println!("Submitting to {peer} …");
    run_external_node(&["submit-tx", "--tx", tx, "--peer", peer])?;
    Ok(())
}

// ── External node launcher ────────────────────────────────────────────────────

pub fn run_external_node(args: &[&str]) -> Result<()> {
    // Prefer sibling binaries from same target dir
    let exe = std::env::current_exe().ok();
    let node = exe
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.join("meshchain-node")));
    if let Some(n) = node {
        if n.exists() {
            let status = Command::new(n).args(args).status()?;
            if !status.success() {
                bail!("meshchain-node failed");
            }
            return Ok(());
        }
    }
    let status = Command::new("meshchain-node").args(args).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => bail!("meshchain-node failed"),
        Err(_) => bail!(
            "Could not find meshchain-node. Build with:\n  cargo build -p mesh -p meshchain-node"
        ),
    }
}
