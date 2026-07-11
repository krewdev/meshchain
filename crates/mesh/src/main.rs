//! MeshChain simple CLI.
//!
//! Designed to be readable without a GUI:
//!   mesh setup
//!   mesh new-wallet
//!   mesh balance
//!   mesh send <who> <amount>
//!   mesh register
//!   mesh new-cold-key
//!   mesh demo
//!   mesh help

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use meshchain_ledger::genesis::ONE_MESH;
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{mesh_name, parse_recipient, short_id, short_id_hex};
use meshchain_proto::crypto::Keypair;
use meshchain_proto::pq::PqKeypair;
use meshchain_proto::tx::{Tx, TxBody};
use meshchain_transport::{fragment_bytes, session_id_from_hash};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "mesh",
    about = "MeshChain — hold and move money on a private radio mesh (simple CLI)",
    long_about = "Simple commands. No app UI required.\n\n\
Examples:\n  \
  mesh testnet-setup      Join MeshChain public testnet profile\n  \
  mesh testnet-info       Show testnet parameters\n  \
  mesh setup              Create a local dev network\n  \
  mesh new-wallet         Make a new spending wallet\n  \
  mesh join-public        Install shared public genesis + seeds\n  \
  mesh register           Put your wallet on-chain (so others can pay you)\n  \
  mesh balance            Show how much you have\n  \
  mesh send BOB 5         Send 5 MESH to Bob’s short address\n  \
  mesh sync-state         Pull chain_state from a public scanner\n  \
  mesh new-cold-key       Make a quantum-safe cold key (keep offline)\n  \
  mesh demo               Run the built-in network demo\n"
)]
struct Cli {
    /// Where network files live (default: ./data)
    #[arg(long, global = true, default_value = "./data")]
    dir: PathBuf,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up the public TESTNET profile (tMESH — no real value)
    #[command(name = "testnet-setup")]
    TestnetSetup {
        #[arg(long, default_value_t = 3)]
        validators: u8,
    },

    /// Show public testnet parameters (chain id, channel, warnings)
    #[command(name = "testnet-info")]
    TestnetInfo,

    /// Install published public genesis + seeds into --dir (join shared testnet)
    #[command(name = "join-public")]
    JoinPublic {
        /// Optional scanner base URL to sync chain_state after install
        #[arg(long)]
        scanner: Option<String>,
    },

    /// Pull chain_state.json from a scanner (light client sync)
    #[command(name = "sync-state")]
    SyncState {
        /// Scanner base, e.g. http://34.172.103.125:8788
        #[arg(long)]
        scanner: String,
    },

    /// Create a local DEV network (not the public testnet)
    Setup {
        /// How many validator computers (default 3)
        #[arg(long, default_value_t = 3)]
        validators: u8,
    },

    /// Run a full demo: transfers, safety checks, vault mint/burn
    Demo {
        #[arg(long, default_value_t = 5)]
        transfers: u32,
    },

    /// Make a new everyday wallet (save the file somewhere safe)
    #[command(name = "new-wallet")]
    NewWallet {
        /// Optional name for the key file (default: wallet.json)
        #[arg(long, default_value = "wallet.json")]
        name: String,
        /// Also sign Register and submit to a validator (on-chain account)
        #[arg(long)]
        publish: bool,
        /// Validator peer for --publish (default: seeds.json / MESH_SUBMIT / 127.0.0.1:9100)
        #[arg(long, default_value = "")]
        submit: String,
    },

    /// Put a wallet on-chain so others can pay it (Register tx)
    #[command(name = "register")]
    Register {
        #[arg(long, default_value = "wallet.json")]
        wallet: String,
        /// Write the signed register tx here
        #[arg(long)]
        out: Option<PathBuf>,
        /// Submit to this validator peer (default: seeds.json / MESH_SUBMIT / 127.0.0.1:9100)
        #[arg(long)]
        submit: Option<String>,
        /// Sign only; do not submit (overrides default submit)
        #[arg(long, default_value_t = false)]
        no_submit: bool,
    },

    /// Make a quantum-safe cold key for large amounts / long-term storage
    #[command(name = "new-cold-key")]
    NewColdKey {
        #[arg(long, default_value = "cold.json")]
        name: String,
    },

    /// Show your mesh name (who people send money to)
    Address {
        #[arg(long, default_value = "wallet.json")]
        wallet: String,
    },

    /// Show how much MESH you have
    Balance {
        #[arg(long, default_value = "wallet.json")]
        wallet: String,
    },

    /// Send MESH to someone (mesh name like M4K7X-J9P2Q-R3W, or hex)
    Send {
        /// Their mesh name (M4K7X-J9P2Q-R3W) or 16-char hex
        to: String,
        /// How much MESH to send (whole number or decimal like 1.5)
        amount: String,
        #[arg(long, default_value = "wallet.json")]
        wallet: String,
        /// Cold key file (needed for large sends)
        #[arg(long, default_value = "cold.json")]
        cold: String,
        /// Priority fee / tip in MESH (paid to block producer for faster inclusion)
        #[arg(long, default_value = "0")]
        fee: String,
        /// Write the signed payment to this file instead of only printing it
        #[arg(long)]
        out: Option<PathBuf>,
        /// Submit to validator peer after signing (e.g. 127.0.0.1:9100)
        #[arg(long)]
        submit: Option<String>,
        /// After --submit, wait and refresh local chain_state from data/v0
        #[arg(long, default_value_t = true)]
        wait: bool,
    },

    /// Show network height and total MESH
    Status,

    /// Quantum cold-storage radio demo (splits a big signature into small radio packets)
    #[command(name = "cold-demo")]
    ColdDemo,

    /// Print plain-English help for cold storage
    #[command(name = "how-cold-works")]
    HowColdWorks,

    /// Show security & privacy posture (what is hardened, what is not)
    #[command(name = "security")]
    Security,

    /// Generate a validator key to apply for a PoA seat (share only public_hex)
    #[command(name = "validator-keygen")]
    ValidatorKeygen {
        /// Key file name under data/keys/
        #[arg(long, default_value = "operator-validator.json")]
        name: String,
    },

    /// Run a testnet validator (must be listed in shared genesis)
    #[command(name = "validator")]
    Validator {
        #[arg(long)]
        index: u8,
        #[arg(long, default_value = "0.0.0.0:9100")]
        listen: String,
        #[arg(long = "peer")]
        peers: Vec<String>,
    },

    /// Run a non-producing full node (anyone — needs shared genesis + seeds)
    #[command(name = "observer")]
    Observer {
        #[arg(long, default_value = "0.0.0.0:9100")]
        listen: String,
        #[arg(long = "peer")]
        peers: Vec<String>,
    },

    /// Show published attestors / Solana devnet program id
    #[command(name = "testnet-attestors")]
    TestnetAttestors,

    /// Start the blockchain scanner (internet explorer UI + API)
    Scanner {
        #[arg(long, default_value = "0.0.0.0:8787")]
        listen: String,
        /// open = public internet | mesh2fa = require mesh signature later
        #[arg(long, default_value = "open")]
        auth: String,
    },
}

fn keys_dir(dir: &Path) -> PathBuf {
    dir.join("keys")
}

fn wallet_path(dir: &Path, name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() || name.contains('/') {
        p
    } else {
        keys_dir(dir).join(name)
    }
}

/// Prefer the freshest chain_state among lab snapshot and live v0 validator tree.
fn best_chain_state_path(dir: &Path) -> PathBuf {
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

fn promote_v0_snapshot(dir: &Path) {
    let v0 = dir.join("v0").join("chain_state.json");
    let snap = dir.join("chain_state.json");
    if v0.exists() {
        if let Err(e) = fs::copy(&v0, &snap) {
            eprintln!("note: could not promote v0 chain_state: {e}");
        }
    }
}

fn is_local_peer(peer: &str) -> bool {
    let host = peer.split(':').next().unwrap_or(peer);
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// Default submit peer: MESH_SUBMIT env → data/seeds.json → repo testnet/seeds.json → localhost
fn default_submit_peer(dir: &Path) -> String {
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
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str())
                                    .collect::<Vec<_>>()
                            })
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

fn default_scanner_url(dir: &Path) -> Option<String> {
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
                if let Some(u) = v
                    .pointer("/http/scanner")
                    .and_then(|x| x.as_str())
                {
                    return Some(u.to_string());
                }
            }
        }
    }
    None
}

fn sync_state_from_scanner(dir: &Path, scanner_base: &str) -> Result<ChainState> {
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

/// Minimal HTTP GET without extra crates (std only).
fn http_get(url: &str) -> Result<String> {
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

fn install_public_artifacts(dir: &Path) -> Result<(String, Option<String>)> {
    fs::create_dir_all(dir)?;
    fs::create_dir_all(keys_dir(dir))?;

    let genesis_candidates = [
        PathBuf::from("testnet/published/genesis.json"),
        PathBuf::from("testnet/genesis.public.json"),
        PathBuf::from("web/testnet/published/genesis.json"),
    ];
    let mut genesis_src = None;
    for p in &genesis_candidates {
        if p.exists() {
            genesis_src = Some(p.clone());
            break;
        }
    }
    let genesis_src = genesis_src.context(
        "No published genesis found. Expected testnet/published/genesis.json (clone full repo).",
    )?;
    fs::copy(&genesis_src, dir.join("genesis.json"))
        .with_context(|| format!("copy {}", genesis_src.display()))?;
    println!("Installed genesis from {}", genesis_src.display());

    let seeds_candidates = [
        PathBuf::from("testnet/seeds.json"),
        PathBuf::from("testnet/published/seeds.json"),
        PathBuf::from("web/testnet/seeds.json"),
    ];
    for p in &seeds_candidates {
        if p.exists() {
            fs::copy(p, dir.join("seeds.json"))?;
            println!("Installed seeds from {}", p.display());
            break;
        }
    }

    // Profile marker for CLI
    let profile = serde_json::json!({
        "is_testnet": true,
        "chain_id": "meshchain-testnet-1",
        "token_symbol": "tMESH",
        "channel_name": "MeshChain-Testnet-1",
        "warning": "TESTNET ONLY — no real value",
        "docs": "https://meshchain-sigma.vercel.app/docs/?doc=RUN_A_NODE",
        "joined_public": true,
    });
    fs::write(
        dir.join("testnet_profile.json"),
        serde_json::to_string_pretty(&profile)?,
    )?;

    let peer = default_submit_peer(dir);
    let scanner = default_scanner_url(dir);
    Ok((peer, scanner))
}

fn refresh_after_submit(dir: &Path, peer: &str) {
    if is_local_peer(peer) {
        thread::sleep(Duration::from_secs(3));
        promote_v0_snapshot(dir);
        return;
    }
    // Remote seed: wait then pull scanner snapshot when available
    thread::sleep(Duration::from_secs(5));
    if let Some(scanner) = default_scanner_url(dir) {
        if let Err(e) = sync_state_from_scanner(dir, &scanner) {
            eprintln!("note: could not sync from scanner ({e}). Run: mesh sync-state --scanner {scanner}");
        }
    } else {
        eprintln!("note: remote submit done. Sync with: mesh sync-state --scanner http://SEED:8788");
    }
}

fn submit_tx_to_peer(tx_path: &Path, peer: &str) -> Result<()> {
    let tx = tx_path
        .to_str()
        .context("tx path is not valid UTF-8")?;
    println!("Submitting to {peer} …");
    run_external_node(&["submit-tx", "--tx", tx, "--peer", peer])?;
    Ok(())
}

fn sign_register(dir: &Path, wallet: &str, out: Option<PathBuf>) -> Result<(PathBuf, Tx)> {
    let wpath = wallet_path(dir, wallet);
    let kp = load_wallet(&wpath)?;
    let pubkey = kp.public_key();
    let sid = short_id(&pubkey);
    let state_path = best_chain_state_path(dir);
    let nonce = if state_path.exists() {
        let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if let Some(acc) = st.account(&sid) {
            if acc.nonce > 0 || acc.balance > 0 {
                bail!(
                    "Wallet {} is already on-chain (balance {}, nonce {}).",
                    mesh_name(&sid),
                    fmt_mesh(acc.balance),
                    acc.nonce
                );
            }
            // Account may exist with 0/0 (edge); still try register with nonce 0
            acc.nonce
        } else {
            0
        }
    } else {
        0
    };

    let body = TxBody::Register { nonce, pubkey };
    let tx = Tx::sign(body, &kp).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    tx.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let out_path = out.unwrap_or_else(|| dir.join("last_register.json"));
    fs::write(&out_path, serde_json::to_string_pretty(&tx)?)?;
    Ok((out_path, tx))
}

fn load_wallet(path: &Path) -> Result<Keypair> {
    let file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(path).with_context(|| {
            format!(
                "Could not open wallet file {}. Try: mesh new-wallet",
                path.display()
            )
        })?)?;
    Keypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn load_cold(path: &Path) -> Result<PqKeypair> {
    let file: meshchain_proto::pq::PqKeypairFile =
        serde_json::from_str(&fs::read_to_string(path).with_context(|| {
            format!(
                "Could not open cold key {}. Try: mesh new-cold-key",
                path.display()
            )
        })?)?;
    PqKeypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn parse_mesh_amount(s: &str) -> Result<u64> {
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

fn fmt_mesh(units: u64) -> String {
    format!("{:.6}", units as f64 / ONE_MESH as f64)
}

fn print_testnet_info(dir: &Path) -> Result<()> {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║         MeshChain PUBLIC TESTNET                 ║");
    println!("║         meshchain-testnet-1                      ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("Status:     active (software testnet)");
    println!("Token:      tMESH — NO CASH VALUE");
    println!("chain_id:   meshchain-testnet-1");
    println!("Channel:    MeshChain-Testnet-1  (private Meshtastic)");
    println!("Solana:     devnet only for bridge experiments");
    println!("Docs:       https://meshchain-sigma.vercel.app/docs/?doc=TESTNET");
    println!("Params:     https://meshchain-sigma.vercel.app/testnet/network.json");
    println!();

    let profile = dir.join("testnet_profile.json");
    let genesis = dir.join("genesis.json");
    if profile.exists() {
        println!("Local profile: {}", profile.display());
        if let Ok(s) = fs::read_to_string(&profile) {
            println!("{s}");
        }
    } else if genesis.exists() {
        if let Ok(s) = fs::read_to_string(&genesis) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                println!(
                    "Local genesis chain_id: {}",
                    v.get("chain_id").and_then(|c| c.as_str()).unwrap_or("?")
                );
            }
        }
        println!("(Run mesh testnet-setup for full testnet profile files.)");
    } else {
        println!("No local data yet. Run:");
        println!("  mesh testnet-setup");
    }

    println!();
    println!("Join steps:");
    println!("  1. mesh testnet-setup");
    println!("  2. mesh demo");
    println!("  3. mesh new-wallet");
    println!("  4. Optional: Meshtastic channel MeshChain-Testnet-1 with test peers");
    println!();
    println!("Never deposit mainnet funds into test software.");
    Ok(())
}

fn run_external_node(args: &[&str]) -> Result<()> {
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = cli.dir;

    match cli.cmd {
        Commands::TestnetSetup { validators } => {
            println!("Setting up MeshChain PUBLIC TESTNET (meshchain-testnet-1)…");
            println!("WARNING: tMESH has no cash value. Balances may be wiped.\n");
            run_external_node(&[
                "init",
                "--data-dir",
                dir.to_str().unwrap_or("./data"),
                "--validators",
                &validators.to_string(),
                "--testnet",
            ])?;
            println!();
            println!("Done. You are on the testnet profile.");
            println!("  mesh testnet-info");
            println!("  mesh demo");
            println!("  mesh new-wallet");
            println!("  Docs: https://meshchain-sigma.vercel.app/docs/?doc=TESTNET");
        }

        Commands::TestnetInfo => {
            print_testnet_info(&dir)?;
        }

        Commands::JoinPublic { scanner } => {
            println!("Joining MeshChain PUBLIC testnet profile…");
            let (peer, scanner_from_seeds) = install_public_artifacts(&dir)?;
            let scanner = scanner.or(scanner_from_seeds);
            if let Some(ref sc) = scanner {
                match sync_state_from_scanner(&dir, sc) {
                    Ok(st) => {
                        println!("Synced live state height={}", st.height);
                    }
                    Err(e) => {
                        eprintln!("WARN: sync-state skipped: {e}");
                        eprintln!("  Retry: mesh sync-state --scanner {sc}");
                    }
                }
            }
            println!();
            println!("Done. Public seed peer: {peer}");
            println!("Next:");
            println!("  mesh new-wallet --name me.json");
            println!("  mesh register --wallet data/keys/me.json --submit {peer}");
            if let Some(sc) = scanner {
                println!("  mesh sync-state --scanner {sc}");
                println!("  Faucet/scanner: see {sc}");
            }
            println!("  Docs: docs/RUN_A_NODE.md");
        }

        Commands::SyncState { scanner } => {
            let st = sync_state_from_scanner(&dir, &scanner)?;
            println!(
                "Network {} · block #{} · supply {}",
                st.chain_id,
                st.height,
                fmt_mesh(st.total_supply)
            );
        }

        Commands::Setup { validators } => {
            println!("Setting up a local MeshChain DEV network (not public testnet)…");
            run_external_node(&[
                "init",
                "--data-dir",
                dir.to_str().unwrap_or("./data"),
                "--validators",
                &validators.to_string(),
            ])?;
            println!();
            println!("Done. Next:");
            println!("  mesh demo              # practice transfers");
            println!("  mesh new-wallet        # make your own wallet");
            println!("  mesh balance --wallet alice.json");
            println!("  For the public testnet instead: mesh testnet-setup");
        }

        Commands::Demo { transfers } => {
            println!("Running network demo (safe test money only)…");
            run_external_node(&[
                "sim",
                "--data-dir",
                dir.to_str().unwrap_or("./data"),
                "--transfers",
                &transfers.to_string(),
            ])?;
            println!();
            println!("Demo finished. Check balances with:");
            println!("  mesh balance --wallet alice.json");
            println!("  mesh balance --wallet bob.json");
        }

        Commands::NewWallet {
            name,
            publish,
            submit,
        } => {
            fs::create_dir_all(keys_dir(&dir))?;
            let path = wallet_path(&dir, &name);
            if path.exists() {
                bail!("File already exists: {}. Pick another --name", path.display());
            }
            let kp = Keypair::generate();
            fs::write(&path, serde_json::to_string_pretty(&kp.to_file())?)?;
            let sid = short_id(&kp.public_key());
            let tag = mesh_name(&sid);
            println!("New wallet created.");
            println!("  File:      {}", path.display());
            println!("  Mesh name: {tag}");
            println!("  (hex id:   {})", short_id_hex(&sid));
            println!();
            let peer = if submit.is_empty() {
                default_submit_peer(&dir)
            } else {
                submit
            };
            if publish {
                let name_only = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&name);
                match sign_register(&dir, name_only, None) {
                    Ok((reg_path, tx)) => {
                        println!("Register signed → {}", reg_path.display());
                        println!("  Id: {}", tx.txid_hex());
                        if let Err(e) = submit_tx_to_peer(&reg_path, &peer) {
                            eprintln!("WARN: submit failed: {e}");
                            eprintln!("  Retry: mesh register --wallet {name_only} --submit {peer}");
                        } else {
                            println!("Submitted Register to {peer}");
                            refresh_after_submit(&dir, &peer);
                            println!("  Check: mesh balance --wallet {name_only}");
                        }
                    }
                    Err(e) => {
                        eprintln!("WARN: could not register: {e}");
                        eprintln!("  Run: mesh register --wallet {name_only} --submit {peer}");
                    }
                }
            } else {
                println!("Share your mesh name so people can pay you.");
                println!("On-chain:  mesh register --wallet {name} --submit {peer}");
                println!("Or:        mesh new-wallet --name {name} --publish");
                println!("Public:    mesh join-public   # once, installs genesis+seeds");
            }
            println!("Keep this file secret. Anyone with it can spend your MESH.");
            println!("For large long-term savings also run: mesh new-cold-key");
        }

        Commands::Register {
            wallet,
            out,
            submit,
            no_submit,
        } => {
            let (out_path, tx) = sign_register(&dir, &wallet, out)?;
            let sid = short_id(&tx.signer);
            println!("Register signed.");
            println!("  Mesh name: {}", mesh_name(&sid));
            println!("  Id:        {}", tx.txid_hex());
            println!("  File:      {}", out_path.display());
            if no_submit {
                let peer = default_submit_peer(&dir);
                println!();
                println!("Signed only. Submit with:");
                println!("  mesh register --wallet {wallet} --submit {peer}");
            } else {
                let peer = submit.unwrap_or_else(|| default_submit_peer(&dir));
                submit_tx_to_peer(&out_path, &peer)?;
                println!("Submitted to {peer}");
                println!("Account will appear after the next block (often a few seconds).");
                refresh_after_submit(&dir, &peer);
            }
        }

        Commands::NewColdKey { name } => {
            fs::create_dir_all(keys_dir(&dir))?;
            let path = wallet_path(&dir, &name);
            if path.exists() {
                bail!("File already exists: {}", path.display());
            }
            let kp = PqKeypair::generate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            fs::write(&path, serde_json::to_string_pretty(&kp.to_file())?)?;
            println!("New quantum-safe cold key created.");
            println!("  File:    {}", path.display());
            println!("  Short:   {}", short_id_hex(&kp.short_id()));
            println!("  Type:    ML-DSA-65 (built for long-term cold storage)");
            println!();
            println!("IMPORTANT:");
            println!("  • Keep this OFF computers with internet or phone signal when you can.");
            println!("  • Copy it to paper/metal backup.");
            println!("  • You need this key for large sends and vault unlocks.");
        }

        Commands::Address { wallet } => {
            let path = wallet_path(&dir, &wallet);
            let kp = load_wallet(&path)?;
            let sid = short_id(&kp.public_key());
            println!("Wallet:    {}", path.display());
            println!("Mesh name: {}", mesh_name(&sid));
            println!("Hex id:    {}", short_id_hex(&sid));
            println!("(Share your mesh name — like M4K7X-J9P2Q-R3W — so people can pay you.)");
        }

        Commands::Balance { wallet } => {
            let path = wallet_path(&dir, &wallet);
            promote_v0_snapshot(&dir);
            let state_path = best_chain_state_path(&dir);
            if !state_path.exists() {
                bail!(
                    "No network state yet. Run:\n  mesh testnet-setup\n  mesh demo\n  # or start validators"
                );
            }
            let kp = load_wallet(&path)?;
            let sid = short_id(&kp.public_key());
            let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let bal = st.balance_of(&sid);
            let nonce = st.account(&sid).map(|a| a.nonce).unwrap_or(0);
            let cold = st.account(&sid).and_then(|a| a.pq_pk.as_ref()).is_some();
            let on_chain = st.account(&sid).is_some();
            println!("Mesh name: {}", mesh_name(&sid));
            println!("Balance:   {} MESH", fmt_mesh(bal));
            println!("Sends:     {nonce} completed from this wallet");
            println!("Network:   block #{} ({})", st.height, state_path.display());
            if !on_chain {
                println!("On-chain:  NO — run: mesh register --wallet {wallet} --submit 127.0.0.1:9100");
            }
            if cold {
                println!("Cold key:  locked to this account (large sends use it)");
            } else {
                println!(
                    "Cold key:  not bound yet (needed for sends ≥ {} MESH)",
                    fmt_mesh(st.pq_required_above)
                );
            }
        }

        Commands::Send {
            to,
            amount,
            wallet,
            cold,
            fee,
            out,
            submit,
            wait,
        } => {
            promote_v0_snapshot(&dir);
            let state_path = best_chain_state_path(&dir);
            if !state_path.exists() {
                bail!("No network yet. Run: mesh setup && mesh demo");
            }
            let wpath = wallet_path(&dir, &wallet);
            let kp = load_wallet(&wpath)?;
            let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let sid = short_id(&kp.public_key());
            let peer_hint = default_submit_peer(&dir);
            let acc = st
                .account(&sid)
                .with_context(|| {
                    format!(
                        "This wallet is not on the network yet. Run:\n  mesh register --wallet {wallet} --submit {peer_hint}"
                    )
                })?;
            let units = parse_mesh_amount(&amount)?;
            let fee_units = parse_mesh_amount(&fee)?;
            let total = units
                .checked_add(fee_units)
                .context("amount + fee overflow")?;
            if total > acc.balance {
                bail!(
                    "Not enough funds. Need {} MESH (amount + priority fee), you have {}.",
                    fmt_mesh(total),
                    fmt_mesh(acc.balance)
                );
            }
            let to_sid = parse_recipient(&to).map_err(|e| anyhow::anyhow!(e))?;
            if st.account(&to_sid).is_none() {
                bail!(
                    "Unknown recipient {}. They must already be on this network.\n  They should run: mesh register --wallet THEIR.json --submit 127.0.0.1:9100",
                    mesh_name(&to_sid)
                );
            }
            let body = TxBody::Transfer {
                nonce: acc.nonce,
                from: sid,
                to: to_sid,
                amount: units,
                fee: fee_units,
            };

            let need_pq = units >= st.pq_required_above;
            let tx = if need_pq {
                println!(
                    "This send is large (≥ {} MESH). Using your cold key…",
                    fmt_mesh(st.pq_required_above)
                );
                let cpath = wallet_path(&dir, &cold);
                let pq = load_cold(&cpath)?;
                Tx::sign_with_pq(body, &kp, &pq).map_err(|e| anyhow::anyhow!(e.to_string()))?
            } else {
                Tx::sign(body, &kp).map_err(|e| anyhow::anyhow!(e.to_string()))?
            };
            tx.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let json = serde_json::to_string_pretty(&tx)?;
            let out_path = out.unwrap_or_else(|| dir.join("last_payment.json"));
            fs::write(&out_path, &json)?;

            println!("Payment signed.");
            println!("  To:     {} ({to})", mesh_name(&to_sid));
            println!("  Amount: {} MESH", fmt_mesh(units));
            if fee_units > 0 {
                println!(
                    "  Fee:    {} MESH priority tip → block producer (faster inclusion)",
                    fmt_mesh(fee_units)
                );
            } else {
                println!("  Fee:    0 (add --fee 0.1 for priority inclusion)");
            }
            println!("  Id:     {}", tx.txid_hex());
            println!("  File:   {}", out_path.display());
            if need_pq {
                let enc = tx.encode().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                let frags = fragment_bytes(session_id_from_hash(&enc), &enc)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                println!(
                    "  Radio:  {} small packets (quantum-safe signature fits on LoRa)",
                    frags.len()
                );
            } else {
                println!("  Radio:  one small packet (everyday send)");
            }
            if let Some(peer) = submit {
                submit_tx_to_peer(&out_path, &peer)?;
                println!("Submitted to {peer}");
                if wait {
                    println!("Waiting for inclusion…");
                    refresh_after_submit(&dir, &peer);
                    if let Ok(st2) = ChainState::load_json(&best_chain_state_path(&dir)) {
                        println!(
                            "Network now block #{} · your balance {} MESH",
                            st2.height,
                            fmt_mesh(st2.balance_of(&sid))
                        );
                    }
                }
            } else {
                let peer = default_submit_peer(&dir);
                println!();
                println!("Signed only. Submit with:");
                println!(
                    "  mesh send {to} {amount} --wallet {wallet} --submit {peer}"
                );
            }
        }

        Commands::Status => {
            promote_v0_snapshot(&dir);
            let state_path = best_chain_state_path(&dir);
            if !state_path.exists() {
                println!("No network data in {}.", dir.display());
                println!("Run: mesh setup");
                return Ok(());
            }
            let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!("Network:   {}", st.chain_id);
            println!("Block:     #{}", st.height);
            println!("State:     {}", state_path.display());
            println!("Total MESH in circulation: {}", fmt_mesh(st.total_supply));
            println!(
                "Large send threshold (needs cold key): {} MESH",
                fmt_mesh(st.pq_required_above)
            );
            println!("Validators: {}", st.validators.len());
        }

        Commands::ColdDemo => {
            println!("Cold storage radio demo…");
            run_external_node(&[
                "pq-cold-demo",
                "--data-dir",
                dir.to_str().unwrap_or("./data"),
            ])?;
        }

        Commands::HowColdWorks => {
            println!(
                r#"
How extreme cold storage works (hybrid lock)
────────────────────────────────────────────
1. On the internet, you lock SOL / dollars / (later BTC) in a vault
   and name your Meshtastic mesh address.
2. The bridge mints the same value as MESH on the radio mesh.
3. You keep a cold key offline (mesh new-cold-key) — not on a phone with 5G.
4. Your radio can stay OFF while you hold. No Wi‑Fi needed.
5. To cash out you need BOTH sides:
   • burn MESH on the mesh (cold key for vault assets)
   • mesh witnesses co-sign unlock on Solana
   Internet alone cannot free the vault.

Everyday small sends use a normal wallet.
Big sends and vault burns need the cold key (ML-DSA-65).

Commands:
  mesh new-wallet
  mesh new-cold-key
  mesh balance
  mesh send <address> <amount>
  mesh cold-demo
  mesh security
"#
            );
        }

        Commands::Security => {
            println!(
                r#"
Security & privacy posture
──────────────────────────
We aim for maximum rigor. We do NOT claim “unhackable” or “perfect anonymity.”

Locked in:
  • Hybrid vault: mesh address + burn id + multiple mesh witnesses
  • Internet-only attackers cannot release vault funds
  • Quantum-safe cold signatures (ML-DSA-65) for large / vault actions
  • Nonces stop double-spend; multi-validator finality
  • Redeem destinations on mesh are hashed (not plain addresses)
  • Fail-secure: if checks fail, money does not move

Privacy:
  • No KYC in the protocol
  • Pseudonymous addresses; optional one-time receive tags
  • Private Meshtastic channel for funds (not public chat)
  • Radio is NOT an anonymity network (physics still applies)

Still your responsibility:
  • Protect cold key backups
  • Diversify mesh witness / validator operators
  • Independent audit before large real value
  • Solana/BTC base chains have their own risks

Read: docs/SECURITY_HARDENING.md  docs/HYBRID_LOCK.md
"#
            );
        }

        Commands::ValidatorKeygen { name } => {
            fs::create_dir_all(keys_dir(&dir))?;
            let path = wallet_path(&dir, &name);
            if path.exists() {
                bail!("File already exists: {}", path.display());
            }
            let kp = Keypair::generate();
            let file = kp.to_file();
            fs::write(&path, serde_json::to_string_pretty(&file)?)?;
            let sid = short_id(&kp.public_key());
            println!("Validator operator key created.");
            println!("  Secret file:  {}  (KEEP PRIVATE)", path.display());
            println!("  public_hex:   {}", file.public_hex);
            println!("  mesh name:    {}", mesh_name(&sid));
            println!();
            println!("This does NOT make you a producer yet.");
            println!("PoA seats are listed in shared genesis.validators.");
            println!();
            println!("Apply (GitHub issue / Discord #validators) with:");
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "role": "validator-applicant",
                    "chain_id": "meshchain-testnet-1",
                    "public_hex": file.public_hex,
                    "operator_name": "your-handle",
                    "contact": "discord-or-email",
                    "listen": "your.host:9100",
                }))?
            );
            println!();
            println!("Docs: docs/RUN_A_NODE.md");
            println!("Template: testnet/operator_application.example.json");
        }

        Commands::Validator {
            index,
            listen,
            peers,
        } => {
            println!("Starting testnet validator index={index} listen={listen}");
            println!("(Must match shared genesis.validators[{index}])");
            let mut args = vec![
                "run".to_string(),
                "--data-dir".into(),
                dir.to_str().unwrap_or("./data").into(),
                "--validator-index".into(),
                index.to_string(),
                "--listen".into(),
                listen,
            ];
            for p in peers {
                args.push("--peer".into());
                args.push(p);
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_external_node(&args_ref)?;
        }

        Commands::Observer { listen, peers } => {
            println!("Starting observer (full node, non-producing) listen={listen}");
            println!("Requires data/genesis.json from the public set + --peer seeds.");
            if peers.is_empty() {
                eprintln!("WARN: no --peer seeds; node will only listen (see testnet/seeds.example.json)");
            }
            let mut args = vec![
                "run".to_string(),
                "--data-dir".into(),
                dir.to_str().unwrap_or("./data").into(),
                "--observer".into(),
                "--listen".into(),
                listen,
            ];
            for p in peers {
                args.push("--peer".into());
                args.push(p);
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_external_node(&args_ref)?;
        }

        Commands::TestnetAttestors => {
            print_attestors()?;
        }

        Commands::Scanner { listen, auth } => {
            println!("Starting MeshChain scanner…");
            println!("Auth mode: {auth} (use mesh2fa later for mesh identity gate)");
            let status = Command::new("meshchain-scanner")
                .args([
                    "--data-dir",
                    dir.to_str().unwrap_or("./data"),
                    "--listen",
                    &listen,
                    "--auth",
                    &auth,
                ])
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(_) => bail!("scanner exited with error"),
                Err(_) => {
                    // try same directory as mesh binary
                    if let Ok(exe) = std::env::current_exe() {
                        if let Some(parent) = exe.parent() {
                            let bin = parent.join("meshchain-scanner");
                            let st = Command::new(bin)
                                .args([
                                    "--data-dir",
                                    dir.to_str().unwrap_or("./data"),
                                    "--listen",
                                    &listen,
                                    "--auth",
                                    &auth,
                                ])
                                .status()?;
                            if !st.success() {
                                bail!("scanner failed — build with: cargo build -p meshchain-scanner");
                            }
                            return Ok(());
                        }
                    }
                    bail!("meshchain-scanner not found. cargo build -p meshchain-scanner");
                }
            }
        }
    }
    Ok(())
}

fn print_attestors() -> Result<()> {
    let paths = [
        PathBuf::from("testnet/attestors.json"),
        PathBuf::from("web/testnet/attestors.json"),
    ];
    for p in &paths {
        if p.exists() {
            println!("{}", fs::read_to_string(p)?);
            return Ok(());
        }
    }
    println!("No local attestors.json — fetch:");
    println!("  https://meshchain-sigma.vercel.app/testnet/attestors.json");
    Ok(())
}
