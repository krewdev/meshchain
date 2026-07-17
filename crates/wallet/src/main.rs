//! MeshChain wallet CLI — keygen, balance, transfer (against local chain state for now).

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use meshchain_ledger::genesis::ONE_MESH;
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{mesh_name, parse_recipient, short_id, short_id_hex};
use meshchain_proto::crypto::Keypair;
use meshchain_proto::pq::{PqKeypair, PqSigned};
use meshchain_proto::tx::{Tx, TxBody};
use meshchain_transport::{fragment_bytes, session_id_from_hash};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "meshchain-wallet", about = "MeshChain wallet")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a classical ed25519 keypair (v1, not quantum-safe)
    Keygen {
        #[arg(long)]
        out: PathBuf,
    },
    /// Generate ML-DSA-65 post-quantum keypair for extreme cold storage (v2)
    PqKeygen {
        #[arg(long)]
        out: PathBuf,
    },
    /// Show address / short id
    Address {
        #[arg(long)]
        key: PathBuf,
    },
    /// Show PQ short id
    PqAddress {
        #[arg(long)]
        key: PathBuf,
    },
    /// Sign an arbitrary message with PQ key (for cold redeem auth demos)
    PqSign {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        message: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Show balance from a chain_state.json snapshot
    Balance {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        state: PathBuf,
    },
    /// Build + sign a transfer (prints JSON tx; does not broadcast yet)
    Transfer {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        state: PathBuf,
        /// Recipient short id (16 hex chars)
        #[arg(long)]
        to: String,
        /// Amount in whole MESH (or use --base-units)
        #[arg(long)]
        amount: Option<u64>,
        #[arg(long)]
        base_units: Option<u64>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn load_key(path: &PathBuf) -> Result<Keypair> {
    let file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(path)?)?;
    Keypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Keygen { out } => {
            let kp = Keypair::generate();
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, serde_json::to_string_pretty(&kp.to_file())?)?;
            let sid = short_id(&kp.public_key());
            println!("wrote {}", out.display());
            println!("scheme:    ed25519 (NOT quantum-safe long-term)");
            println!("mesh name: {}", mesh_name(&sid));
            println!("hex id:    {}", short_id_hex(&sid));
            println!("public:    {}", hex::encode(kp.public_key()));
        }
        Commands::PqKeygen { out } => {
            let kp = PqKeypair::generate().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out, serde_json::to_string_pretty(&kp.to_file())?)?;
            let sid = kp.short_id();
            println!("wrote {}", out.display());
            println!("scheme:  ml-dsa-65 (FIPS 204) — quantum-resistant signatures");
            println!("public:  {} bytes", kp.public_key_bytes().len());
            println!("short:   {}", short_id_hex(&sid));
            println!("note:    store offline; never put secret on internet/5G devices");
        }
        Commands::Address { key } => {
            let kp = load_key(&key)?;
            let sid = short_id(&kp.public_key());
            println!("mesh name: {}", mesh_name(&sid));
            println!("hex id:    {}", short_id_hex(&sid));
            println!("public:    {}", hex::encode(kp.public_key()));
        }
        Commands::PqAddress { key } => {
            let file: meshchain_proto::pq::PqKeypairFile =
                serde_json::from_str(&fs::read_to_string(&key)?)?;
            let kp = PqKeypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!("scheme:  ml-dsa-65");
            println!("short:   {}", short_id_hex(&kp.short_id()));
            println!(
                "pk_hex:  {}...(truncated)",
                &hex::encode(kp.public_key_bytes())[..32]
            );
        }
        Commands::PqSign { key, message, out } => {
            let file: meshchain_proto::pq::PqKeypairFile =
                serde_json::from_str(&fs::read_to_string(&key)?)?;
            let kp = PqKeypair::from_file(&file).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let env = PqSigned::sign_message(message.as_bytes(), &kp)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            env.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let encoded = env.encode().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let sid = session_id_from_hash(&encoded);
            let frags =
                fragment_bytes(sid, &encoded).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            println!(
                "pq envelope {} bytes → {} mesh frames",
                encoded.len(),
                frags.len()
            );
            if let Some(path) = out {
                fs::write(&path, serde_json::to_string_pretty(&env)?)?;
                println!("wrote {}", path.display());
            }
        }
        Commands::Balance { key, state } => {
            let kp = load_key(&key)?;
            let sid = short_id(&kp.public_key());
            let st = ChainState::load_json(&state).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let bal = st.balance_of(&sid);
            let nonce = st.account(&sid).map(|a| a.nonce).unwrap_or(0);
            println!("mesh name: {}", mesh_name(&sid));
            println!("hex id:    {}", short_id_hex(&sid));
            println!(
                "balance:   {} base units ({:.6} MESH)",
                bal,
                bal as f64 / ONE_MESH as f64
            );
            println!("nonce:    {nonce}");
            println!("height:   {}", st.height);
            println!("supply:   {}", st.total_supply);
        }
        Commands::Transfer {
            key,
            state,
            to,
            amount,
            base_units,
            out,
        } => {
            let kp = load_key(&key)?;
            let sid = short_id(&kp.public_key());
            let st = ChainState::load_json(&state).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let acc = st
                .account(&sid)
                .with_context(|| format!("account {} not on chain", short_id_hex(&sid)))?;
            let units = match (base_units, amount) {
                (Some(u), _) => u,
                (None, Some(m)) => m.saturating_mul(ONE_MESH),
                _ => bail!("provide --amount MESH or --base-units"),
            };
            if units > acc.balance {
                bail!("insufficient balance");
            }
            let to_sid = parse_recipient(&to).map_err(|e| anyhow::anyhow!(e))?;
            if st.account(&to_sid).is_none() {
                bail!("recipient not registered on chain");
            }
            let body = TxBody::Transfer {
                nonce: acc.nonce,
                from: sid,
                to: to_sid,
                amount: units,
                fee: 0,
            };
            let tx = Tx::sign(body, &kp).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            tx.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let json = serde_json::to_string_pretty(&tx)?;
            if let Some(path) = out {
                fs::write(&path, &json)?;
                println!("wrote signed tx {}", path.display());
            } else {
                println!("{json}");
            }
            println!("txid: {}", tx.txid_hex());
        }
    }
    Ok(())
}
