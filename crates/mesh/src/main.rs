//! MeshChain simple CLI.
//!
//! Designed to be readable without a GUI:
//!   mesh setup
//!   mesh new-wallet
//!   mesh balance
//!   mesh send <who> <amount>
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
  mesh balance            Show how much you have\n  \
  mesh send BOB 5         Send 5 MESH to Bob’s short address\n  \
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
        /// Write the signed payment to this file instead of only printing it
        #[arg(long)]
        out: Option<PathBuf>,
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

    /// Run a testnet validator process (multi-machine TCP gossip)
    #[command(name = "validator")]
    Validator {
        #[arg(long)]
        index: u8,
        #[arg(long, default_value = "0.0.0.0:9100")]
        listen: String,
        #[arg(long = "peer")]
        peers: Vec<String>,
    },

    /// Show published attestors / Solana devnet program id
    #[command(name = "testnet-attestors")]
    TestnetAttestors,
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

        Commands::NewWallet { name } => {
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
            println!("Share your mesh name so people can pay you.");
            println!("Keep this file secret. Anyone with it can spend your MESH.");
            println!("For large long-term savings also run: mesh new-cold-key");
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
            let state_path = dir.join("chain_state.json");
            if !state_path.exists() {
                bail!(
                    "No network state yet. Run:\n  mesh testnet-setup\n  mesh demo"
                );
            }
            let kp = load_wallet(&path)?;
            let sid = short_id(&kp.public_key());
            let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let bal = st.balance_of(&sid);
            let nonce = st.account(&sid).map(|a| a.nonce).unwrap_or(0);
            let cold = st.account(&sid).and_then(|a| a.pq_pk.as_ref()).is_some();
            println!("Mesh name: {}", mesh_name(&sid));
            println!("Balance:   {} MESH", fmt_mesh(bal));
            println!("Sends:     {nonce} completed from this wallet");
            println!("Network:   block #{}", st.height);
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
            out,
        } => {
            let state_path = dir.join("chain_state.json");
            if !state_path.exists() {
                bail!("No network yet. Run: mesh setup && mesh demo");
            }
            let wpath = wallet_path(&dir, &wallet);
            let kp = load_wallet(&wpath)?;
            let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let sid = short_id(&kp.public_key());
            let acc = st
                .account(&sid)
                .with_context(|| "This wallet is not on the network yet. Get some MESH first.")?;
            let units = parse_mesh_amount(&amount)?;
            if units > acc.balance {
                bail!(
                    "Not enough funds. You have {} MESH.",
                    fmt_mesh(acc.balance)
                );
            }
            let to_sid = parse_recipient(&to).map_err(|e| anyhow::anyhow!(e))?;
            if st.account(&to_sid).is_none() {
                bail!(
                    "Unknown recipient {}. They must already be on this network.",
                    mesh_name(&to_sid)
                );
            }
            let body = TxBody::Transfer {
                nonce: acc.nonce,
                from: sid,
                to: to_sid,
                amount: units,
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
            println!();
            println!("Note: a live radio network will broadcast this file. For now it is saved locally.");
        }

        Commands::Status => {
            let state_path = dir.join("chain_state.json");
            if !state_path.exists() {
                println!("No network data in {}.", dir.display());
                println!("Run: mesh setup");
                return Ok(());
            }
            let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!("Network:   {}", st.chain_id);
            println!("Block:     #{}", st.height);
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

        Commands::Validator {
            index,
            listen,
            peers,
        } => {
            println!("Starting testnet validator index={index} listen={listen}");
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

        Commands::TestnetAttestors => {
            print_attestors()?;
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
