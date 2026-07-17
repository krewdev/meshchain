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
//!
//! Command implementations live in the `cmd` sub-modules:
//!   cmd/wallet.rs   — wallet & key management
//!   cmd/network.rs  — node setup, sync, faucet, validators
//!   cmd/radio.rs    — send, air-submit, cold path
//!   cmd/info.rs     — security, demo
//!   cmd/scanner.rs  — blockchain explorer
//!
//! Shared utilities (path helpers, HTTP, amount parsing) are in `helpers.rs`.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cmd;
mod config;
mod helpers;

use config::MeshConfig;

// ── CLI root ──────────────────────────────────────────────────────────────────

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
  mesh send BOB 5         Send 5 MESH to Bob's short address\n  \
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

// ── Config sub-commands ───────────────────────────────────────────────────────

#[derive(Subcommand)]
enum ConfigAction {
    /// Initialize default settings file (`data/config.json`)
    Init,
    /// Show current zero-config settings
    Show,
    /// Set a configuration value
    Set {
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        delay_ms: Option<u64>,
        #[arg(long)]
        portnum: Option<u32>,
        #[arg(long)]
        compress: Option<bool>,
        #[arg(long)]
        wallet: Option<String>,
    },
}

// ── Radio sub-commands ────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum RadioAction {
    /// Launch the stdio radio bridge (`meshtastic_bridge.py`) with pacing & port filtering
    Bridge {
        #[arg(long)]
        port: Option<String>,
        #[arg(long)]
        delay_ms: Option<u64>,
        #[arg(long)]
        portnum: Option<u32>,
    },
    /// Query attached Meshtastic device over serial/USB using `native_proto`
    Info {
        #[arg(long)]
        port: Option<String>,
    },
    /// Send signed payment directly over the radio
    Send {
        /// Their mesh name (M4K7X-J9P2Q-R3W) or 16-char hex
        to: String,
        /// How much MESH to send
        amount: String,
        #[arg(long)]
        wallet: Option<String>,
        #[arg(long)]
        cold: Option<String>,
        #[arg(long)]
        port: Option<String>,
    },
}

// ── Top-level commands ────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum Commands {
    /// Manage persistent zero-config settings (default port, delay, wallet)
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage attached LoRa radio (bridge, device info, send direct)
    Radio {
        #[command(subcommand)]
        action: RadioAction,
    },

    /// Run automated hardware, environment, and key diagnostics
    Doctor,

    /// Live terminal dashboard of blockchain status and radio metrics
    Monitor,

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
        /// Scanner base (default: seeds.json scanner_https / MESH_SCANNER)
        #[arg(long, default_value = "")]
        scanner: String,
    },

    /// Request tMESH from the public faucet (testnet only)
    #[command(name = "faucet-drip")]
    FaucetDrip {
        #[arg(long, default_value = "wallet.json")]
        wallet: String,
        /// Faucet base URL (default: seeds.json faucet_https)
        #[arg(long, default_value = "")]
        faucet: String,
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
        /// Meshtastic air path: submit via MC frame (tx_air + MCHEX) to peer or radio relay
        #[arg(long, default_value_t = false)]
        air: bool,
        /// Radio relay inject address (default: submit peer or 127.0.0.1:9199)
        #[arg(long, default_value = "")]
        relay: String,
    },

    /// Submit last_payment.json over Meshtastic air path (MC frame → relay/validator)
    #[command(name = "air-submit")]
    AirSubmit {
        /// Signed tx JSON (default data/last_payment.json)
        #[arg(long, default_value = "")]
        tx: String,
        /// Validator or radio-relay host:port
        #[arg(long, default_value = "")]
        peer: String,
        /// Alias for --peer (radio relay listen, default 127.0.0.1:9199)
        #[arg(long, default_value = "")]
        relay: String,
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

    /// Extend genesis with new validator public keys (coordinator tool; testnet restack)
    #[command(name = "genesis-extend")]
    GenesisExtend {
        /// Existing genesis.json
        #[arg(long)]
        genesis: PathBuf,
        /// New validator public_hex (repeatable)
        #[arg(long = "add")]
        add: Vec<String>,
        /// Output path
        #[arg(long)]
        out: PathBuf,
    },

    /// Coordinator: append public_hex validators to a genesis file (PoA set change)
    #[command(name = "genesis-add")]
    GenesisAdd {
        /// Input genesis.json
        #[arg(long)]
        genesis: PathBuf,
        /// Output path for new genesis
        #[arg(long)]
        out: PathBuf,
        /// public_hex to add (repeatable)
        #[arg(long = "add")]
        add: Vec<String>,
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

// ── Config command handler (stays in main — it's only 3 variants) ─────────────

fn run_config_cmd(dir: &std::path::Path, action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Init => {
            let cfg = MeshConfig::load_or_default(dir);
            let path = cfg.save(dir)?;
            println!("Initialized configuration file: {}", path.display());
            println!("Settings:\n{}", serde_json::to_string_pretty(&cfg)?);
        }
        ConfigAction::Show => {
            let cfg = MeshConfig::load_or_default(dir);
            let path = MeshConfig::config_path(dir);
            println!("Configuration file: {}", path.display());
            if !path.exists() {
                println!("(Using defaults — run 'mesh config init' to save to disk)");
            }
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
        ConfigAction::Set {
            port,
            delay_ms,
            portnum,
            compress,
            wallet,
        } => {
            let mut cfg = MeshConfig::load_or_default(dir);
            if let Some(p) = port {
                cfg.radio_port = if p == "auto" || p.is_empty() {
                    None
                } else {
                    Some(p)
                };
            }
            if let Some(d) = delay_ms {
                cfg.tx_delay_ms = d;
            }
            if let Some(pn) = portnum {
                cfg.portnum = pn;
            }
            if let Some(c) = compress {
                cfg.compression = c;
            }
            if let Some(w) = wallet {
                cfg.default_wallet = w;
            }
            let path = cfg.save(dir)?;
            println!("Updated configuration saved to: {}", path.display());
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
    }
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = cli.dir;

    match cli.cmd {
        Commands::Config { action } => run_config_cmd(&dir, action)?,
        Commands::Radio { .. } => bail!("Radio commands not implemented yet"),
        Commands::Doctor => bail!("Doctor not implemented yet"),
        Commands::Monitor => bail!("Monitor not implemented yet"),

        Commands::TestnetSetup { validators } => {
            cmd::network::cmd_testnet_setup(&dir, validators)?;
        }
        Commands::TestnetInfo => {
            cmd::network::cmd_testnet_info(&dir)?;
        }
        Commands::JoinPublic { scanner } => {
            cmd::network::cmd_join_public(&dir, scanner)?;
        }
        Commands::SyncState { scanner } => {
            cmd::network::cmd_sync_state(&dir, &scanner)?;
        }
        Commands::FaucetDrip { wallet, faucet } => {
            cmd::network::cmd_faucet_drip(&dir, &wallet, &faucet)?;
        }
        Commands::Setup { validators } => {
            cmd::network::cmd_setup(&dir, validators)?;
        }
        Commands::Demo { transfers } => {
            cmd::info::cmd_demo(&dir, transfers)?;
        }
        Commands::Status => {
            cmd::network::cmd_status(&dir)?;
        }
        Commands::Validator {
            index,
            listen,
            peers,
        } => {
            cmd::network::cmd_validator(&dir, index, &listen, &peers)?;
        }
        Commands::Observer { listen, peers } => {
            cmd::network::cmd_observer(&dir, &listen, &peers)?;
        }
        Commands::TestnetAttestors => {
            cmd::network::cmd_testnet_attestors()?;
        }
        Commands::GenesisExtend { genesis, add, out }
        | Commands::GenesisAdd { genesis, out, add } => {
            cmd::network::cmd_genesis_modify(&genesis, &add, &out)?;
        }

        Commands::NewWallet {
            name,
            publish,
            submit,
        } => {
            cmd::wallet::cmd_new_wallet(&dir, &name, publish, &submit)?;
        }
        Commands::Register {
            wallet,
            out,
            submit,
            no_submit,
        } => {
            cmd::wallet::cmd_register(&dir, &wallet, out, submit, no_submit)?;
        }
        Commands::NewColdKey { name } => {
            cmd::wallet::cmd_new_cold_key(&dir, &name)?;
        }
        Commands::Address { wallet } => {
            cmd::wallet::cmd_address(&dir, &wallet)?;
        }
        Commands::Balance { wallet } => {
            cmd::wallet::cmd_balance(&dir, &wallet)?;
        }
        Commands::ValidatorKeygen { name } => {
            cmd::wallet::cmd_validator_keygen(&dir, &name)?;
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
            air,
            relay,
        } => {
            cmd::radio::cmd_send(
                &dir, &to, &amount, &wallet, &cold, &fee, out, submit, wait, air, &relay,
            )?;
        }
        Commands::AirSubmit { tx, peer, relay } => {
            cmd::radio::cmd_air_submit(&dir, &tx, &peer, &relay)?;
        }
        Commands::ColdDemo => {
            cmd::radio::cmd_cold_demo(&dir)?;
        }
        Commands::HowColdWorks => {
            cmd::radio::cmd_how_cold_works();
        }

        Commands::Security => {
            cmd::info::cmd_security();
        }

        Commands::Scanner { listen, auth } => {
            cmd::scanner::cmd_scanner(&dir, &listen, &auth)?;
        }
    }
    Ok(())
}
