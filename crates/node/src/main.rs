//! MeshChain node: PoA simulator and single-process multi-validator demo.

mod consensus;
mod sim;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use meshchain_ledger::genesis::{GenesisAccount, GenesisConfig, ONE_MESH};
use meshchain_proto::crypto::Keypair;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(name = "meshchain-node", about = "MeshChain validator / simulator")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate genesis + N validator keypairs for local dev
    Init {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        #[arg(long, default_value_t = 3)]
        validators: u8,
        /// Faucet balance in whole MESH
        #[arg(long, default_value_t = 1_000_000)]
        faucet_mesh: u64,
    },
    /// Run in-process multi-validator simulation (no radios)
    Sim {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        /// Number of transfers to execute in the demo
        #[arg(long, default_value_t = 5)]
        transfers: u32,
    },
    /// Demo PQ cold-storage auth over fragmented mesh frames (sim bus)
    PqColdDemo {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
    },
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Init {
            data_dir,
            validators,
            faucet_mesh,
        } => {
            fs::create_dir_all(&data_dir)?;
            let keys_dir = data_dir.join("keys");
            fs::create_dir_all(&keys_dir)?;

            let mut validator_hex = Vec::new();
            let mut validator_kps = Vec::new();
            for i in 0..validators {
                let kp = Keypair::generate();
                let file = kp.to_file();
                let path = keys_dir.join(format!("validator-{i}.json"));
                fs::write(&path, serde_json::to_string_pretty(&file)?)?;
                validator_hex.push(file.public_hex.clone());
                validator_kps.push(kp);
                println!("validator-{i}: {}", file.public_hex);
            }

            // Faucet / demo user
            let faucet = Keypair::generate();
            let faucet_file = faucet.to_file();
            fs::write(
                keys_dir.join("faucet.json"),
                serde_json::to_string_pretty(&faucet_file)?,
            )?;
            println!("faucet:    {}", faucet_file.public_hex);

            // Alice and Bob for demo
            let alice = Keypair::generate();
            let bob = Keypair::generate();
            fs::write(
                keys_dir.join("alice.json"),
                serde_json::to_string_pretty(&alice.to_file())?,
            )?;
            fs::write(
                keys_dir.join("bob.json"),
                serde_json::to_string_pretty(&bob.to_file())?,
            )?;
            println!("alice:     {}", hex::encode(alice.public_key()));
            println!("bob:       {}", hex::encode(bob.public_key()));

            let genesis = GenesisConfig {
                chain_id: "meshchain-dev".into(),
                validators: validator_hex,
                block_reward: 100_000,
                allocations: vec![
                    GenesisAccount {
                        public_key_hex: faucet_file.public_hex,
                        balance: faucet_mesh.saturating_mul(ONE_MESH),
                    },
                    GenesisAccount {
                        public_key_hex: hex::encode(alice.public_key()),
                        balance: 10_000 * ONE_MESH,
                    },
                    GenesisAccount {
                        public_key_hex: hex::encode(bob.public_key()),
                        balance: 0,
                    },
                ],
                minters: vec![], // validators auto-added as minters
                slot_secs: 5,   // fast for sim
                // Big moves need cold (quantum-safe) key. Small demo transfers stay simple.
                pq_required_above: 100 * ONE_MESH,
            };

            let genesis_path = data_dir.join("genesis.json");
            fs::write(&genesis_path, serde_json::to_string_pretty(&genesis)?)?;
            println!("wrote {}", genesis_path.display());
            println!("init complete — run: meshchain-node sim --data-dir {}", data_dir.display());
        }
        Commands::Sim { data_dir, transfers } => {
            sim::run_sim(&data_dir, transfers, now_secs()).context("sim failed")?;
        }
        Commands::PqColdDemo { data_dir } => {
            sim::run_pq_cold_demo(&data_dir).context("pq cold demo failed")?;
        }
    }
    Ok(())
}
