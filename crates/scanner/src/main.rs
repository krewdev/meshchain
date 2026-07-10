//! MeshChain blockchain scanner — public HTTP explorer for testnet.
//!
//! Internet-accessible by default (`--auth open`).
//! Later: `--auth mesh2fa` requires a signed mesh challenge (stubbed).

mod auth;
mod http;
mod model;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use meshchain_ledger::state::ChainState;

#[derive(Parser, Debug)]
#[command(
    name = "meshchain-scanner",
    about = "MeshChain testnet scanner — browse blocks, accounts, mesh names"
)]
struct Cli {
    /// Bind address (0.0.0.0 for internet access)
    #[arg(long, default_value = "0.0.0.0:8787")]
    listen: String,

    /// Directory with chain_state.json (and optional network.json)
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,

    /// Auth mode: open (public) | mesh2fa (require mesh signature — stub for now)
    #[arg(long, default_value = "open")]
    auth: String,

    /// Reload chain_state.json from disk every N seconds (0 = only on each request)
    #[arg(long, default_value_t = 5)]
    reload_secs: u64,
}

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub auth_mode: auth::AuthMode,
    pub chain: Arc<RwLock<ChainState>>,
    pub network_meta: Arc<RwLock<serde_json::Value>>,
    pub started_unix: u64,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn load_chain(data_dir: &std::path::Path) -> Result<ChainState> {
    let path = data_dir.join("chain_state.json");
    ChainState::load_json(&path).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}

fn load_network_meta(data_dir: &std::path::Path) -> serde_json::Value {
    for p in [
        data_dir.join("network.json"),
        data_dir.join("testnet_profile.json"),
        PathBuf::from("testnet/network.json"),
        PathBuf::from("web/testnet/network.json"),
    ] {
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str(&s) {
                return v;
            }
        }
    }
    serde_json::json!({
        "chain_id": "meshchain-testnet-1",
        "is_testnet": true,
        "warning": "TESTNET ONLY"
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let auth_mode = auth::AuthMode::parse(&cli.auth)?;
    let listen: SocketAddr = cli
        .listen
        .parse()
        .with_context(|| format!("bad --listen {}", cli.listen))?;

    let chain = load_chain(&cli.data_dir).with_context(|| {
        format!(
            "load chain_state from {} (run: mesh testnet-setup && mesh demo)",
            cli.data_dir.display()
        )
    })?;
    let network_meta = load_network_meta(&cli.data_dir);

    let state = AppState {
        data_dir: cli.data_dir.clone(),
        auth_mode,
        chain: Arc::new(RwLock::new(chain)),
        network_meta: Arc::new(RwLock::new(network_meta)),
        started_unix: now_unix(),
    };

    // Background reload
    if cli.reload_secs > 0 {
        let st = state.clone();
        let secs = cli.reload_secs;
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            if let Ok(c) = load_chain(&st.data_dir) {
                if let Ok(mut w) = st.chain.write() {
                    *w = c;
                }
            }
            let meta = load_network_meta(&st.data_dir);
            if let Ok(mut w) = st.network_meta.write() {
                *w = meta;
            }
        });
    }

    println!("╔══════════════════════════════════════════════════╗");
    println!("║     MeshChain Scanner (testnet explorer)         ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!("listen:  http://{listen}");
    println!("data:    {}", cli.data_dir.display());
    println!("auth:    {:?} (mesh2fa ready later)", state.auth_mode);
    {
        let c = state.chain.read().unwrap();
        println!(
            "chain:   {}  height={}  accounts={}  supply={}",
            c.chain_id,
            c.height,
            c.accounts.len(),
            c.total_supply
        );
    }
    println!("UI:      http://{listen}/");
    println!("API:     http://{listen}/api/v1/status");
    println!();

    http::serve(listen, state)
}
