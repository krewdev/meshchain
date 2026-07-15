//! MeshChain node: PoA simulator and single-process multi-validator demo.

mod consensus;
mod net;
mod run;
mod sim;
mod sync_validate;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use meshchain_ledger::genesis::{GenesisAccount, GenesisConfig, ONE_MESH};
use meshchain_proto::crypto::Keypair;
use std::fs;
use std::net::SocketAddr;
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
    /// Generate genesis + N validator keypairs for local dev or public testnet profile
    Init {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        #[arg(long, default_value_t = 3)]
        validators: u8,
        /// Faucet balance in whole MESH / tMESH
        #[arg(long, default_value_t = 1_000_000)]
        faucet_mesh: u64,
        /// Use public testnet profile (chain_id meshchain-testnet-1)
        #[arg(long, default_value_t = false)]
        testnet: bool,
        /// Override chain id (default: meshchain-dev or meshchain-testnet-1)
        #[arg(long)]
        chain_id: Option<String>,
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
    /// Run a multi-machine validator or observer (TCP gossip)
    Run {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        /// Index into genesis.validators (0..N-1). Required unless --observer.
        #[arg(long)]
        validator_index: Option<u8>,
        /// Non-producing full node: relay + follow chain (anyone can run)
        #[arg(long, default_value_t = false)]
        observer: bool,
        /// Listen address, e.g. 0.0.0.0:9100
        #[arg(long, default_value = "0.0.0.0:9100")]
        listen: String,
        /// Bootstrap peers host:port (repeatable)
        #[arg(long = "peer")]
        peers: Vec<String>,
        #[arg(long, default_value_t = 100)]
        slot_ms: u64,
        /// LoRa radio port for live Meshtastic bridging, e.g. /dev/ttyUSB0 or tcp:localhost:4403
        #[arg(long)]
        radio_port: Option<String>,
    },
    /// Submit a signed payment JSON to a validator peer
    SubmitTx {
        #[arg(long)]
        tx: PathBuf,
        #[arg(long, default_value = "127.0.0.1:9100")]
        peer: String,
        /// Also send Meshtastic air frames (tx_air + MCHEX) for radio relay path
        #[arg(long, default_value_t = false)]
        air: bool,
    },
    /// Relayer helper: after Solana vault deposit, mint tMESH to a mesh pubkey
    MintForDeposit {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        /// Recipient ed25519 public key hex (32 bytes)
        #[arg(long)]
        to_pubkey: String,
        /// Amount in tMESH base units (usually = net lamports from deposit)
        #[arg(long)]
        amount: u64,
        /// 16-byte external ref as 32 hex chars (hash of sol tx)
        #[arg(long)]
        external_ref_hex: String,
        #[arg(long, default_value_t = 0)]
        validator_index: u8,
        /// Submit Mint tx to a live peer (required for multi-validator hosts)
        #[arg(long, default_value = "")]
        peer: String,
        /// Offline local finality (LAB ONLY — can fork multi-process hosts)
        #[arg(long, default_value_t = false)]
        offline: bool,
    },
    /// Burn vault-linked tMESH (needs cold PQ key); prints burn txid for Solana withdraw
    BurnForWithdraw {
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        cold: PathBuf,
        /// Amount in tMESH base units
        #[arg(long)]
        amount: u64,
        /// Solana destination pubkey base58 (hashed into redeem_hint)
        #[arg(long)]
        dest_sol: String,
        #[arg(long, default_value_t = 1)]
        asset_id: u32,
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
            testnet,
            chain_id,
        } => {
            fs::create_dir_all(&data_dir)?;
            let keys_dir = data_dir.join("keys");
            fs::create_dir_all(&keys_dir)?;

            let chain = chain_id.unwrap_or_else(|| {
                if testnet {
                    "meshchain-testnet-1".into()
                } else {
                    "meshchain-dev".into()
                }
            });

            if testnet {
                println!("╔══════════════════════════════════════════════╗");
                println!("║  MeshChain PUBLIC TESTNET                    ║");
                println!("║  chain_id = meshchain-testnet-1              ║");
                println!("║  tMESH has NO cash value — may be wiped      ║");
                println!("╚══════════════════════════════════════════════╝");
            }

            let mut validator_hex = Vec::new();
            for i in 0..validators {
                let kp = Keypair::generate();
                let file = kp.to_file();
                let path = keys_dir.join(format!("validator-{i}.json"));
                fs::write(&path, serde_json::to_string_pretty(&file)?)?;
                validator_hex.push(file.public_hex.clone());
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
                chain_id: chain.clone(),
                validators: validator_hex,
                block_reward: 100_000,
                allocations: vec![
                    GenesisAccount {
                        public_key_hex: faucet_file.public_hex.clone(),
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
                slot_secs: if testnet { 30 } else { 5 },
                // Big moves need cold (quantum-safe) key. Small demo transfers stay simple.
                protocol_version: 1,
            pq_required_above: 100 * ONE_MESH,
            };

            let genesis_path = data_dir.join("genesis.json");
            fs::write(&genesis_path, serde_json::to_string_pretty(&genesis)?)?;

            // Write testnet profile marker for CLI / site alignment
            if testnet || chain == "meshchain-testnet-1" {
                let profile = serde_json::json!({
                    "is_testnet": true,
                    "chain_id": chain,
                    "token_symbol": "tMESH",
                    "channel_name": "MeshChain-Testnet-1",
                    "solana_cluster": "devnet",
                    "warning": "TESTNET ONLY — no real value",
                    "docs": "https://meshchain-sigma.vercel.app/docs/?doc=TESTNET",
                    "network_json": "https://meshchain-sigma.vercel.app/testnet/network.json",
                });
                fs::write(
                    data_dir.join("testnet_profile.json"),
                    serde_json::to_string_pretty(&profile)?,
                )?;
                // Copy canonical network.json if present in repo
                let canonical = PathBuf::from("testnet/network.json");
                if canonical.exists() {
                    let _ = fs::copy(&canonical, data_dir.join("network.json"));
                }
            }

            println!("wrote {}", genesis_path.display());
            println!("chain_id: {chain}");
            if testnet {
                println!("token: tMESH (test only)");
                println!("channel: MeshChain-Testnet-1");
                println!("next: mesh testnet-info   OR   mesh demo");
            } else {
                println!("init complete — run: meshchain-node sim --data-dir {}", data_dir.display());
            }
        }
        Commands::Sim { data_dir, transfers } => {
            sim::run_sim(&data_dir, transfers, now_secs()).context("sim failed")?;
        }
        Commands::PqColdDemo { data_dir } => {
            sim::run_pq_cold_demo(&data_dir).context("pq cold demo failed")?;
        }
        Commands::Run {
            data_dir,
            validator_index,
            observer,
            listen,
            peers,
            slot_ms,
            radio_port,
        } => {
            let listen: SocketAddr = listen.parse().context("bad --listen address")?;
            if !observer && validator_index.is_none() {
                anyhow::bail!("provide --validator-index N  or  --observer");
            }
            run::run_validator(run::RunConfig {
                data_dir,
                validator_index,
                observer,
                listen,
                peers,
                slot_ms,
                radio_port,
            })?;
        }
        Commands::SubmitTx { tx, peer, air } => {
            if air {
                run::submit_tx_air(&tx, &peer)?;
            } else {
                run::submit_tx_file(&tx, &peer)?;
            }
        }
        Commands::MintForDeposit {
            data_dir,
            to_pubkey,
            amount,
            external_ref_hex,
            validator_index,
            peer,
            offline,
        } => {
            mint_for_deposit(
                &data_dir,
                &to_pubkey,
                amount,
                &external_ref_hex,
                validator_index,
                &peer,
                offline,
            )?;
        }
        Commands::BurnForWithdraw {
            data_dir,
            wallet,
            cold,
            amount,
            dest_sol,
            asset_id,
        } => {
            burn_for_withdraw(&data_dir, &wallet, &cold, amount, &dest_sol, asset_id)?;
        }
    }
    Ok(())
}

fn mint_for_deposit(
    data_dir: &std::path::Path,
    to_pubkey_hex: &str,
    amount: u64,
    external_ref_hex: &str,
    validator_index: u8,
    peer: &str,
    offline: bool,
) -> Result<()> {
    use meshchain_ledger::state::ChainState;
    use meshchain_proto::address::{short_id, short_id_hex};
    use meshchain_proto::tx::{Tx, TxBody};
    use crate::consensus::{leader_index, produce_block, FinalityTracker};

    let genesis: GenesisConfig = serde_json::from_str(&fs::read_to_string(
        data_dir.join("genesis.json"),
    )?)?;
    let state_path = data_dir.join("chain_state.json");
    let mut state =
        ChainState::from_genesis(&genesis).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    if state_path.exists() {
        if let Ok(prev) = ChainState::load_json(&state_path) {
            if prev.chain_id == state.chain_id && prev.validators == state.validators {
                state = prev;
            } else {
                eprintln!("note: resetting chain_state (genesis/keys changed)");
            }
        }
    }

    let to_bytes = hex::decode(to_pubkey_hex.trim())?;
    if to_bytes.len() != 32 {
        anyhow::bail!("to-pubkey must be 32 bytes hex");
    }
    let mut to_pk = [0u8; 32];
    to_pk.copy_from_slice(&to_bytes);
    let to_sid = short_id(&to_pk);
    state.ensure_account(&to_pk);

    let ref_bytes = hex::decode(external_ref_hex.trim())?;
    if ref_bytes.len() != 16 {
        anyhow::bail!("external-ref-hex must be 16 bytes (32 hex chars)");
    }
    let mut external_ref = [0u8; 16];
    external_ref.copy_from_slice(&ref_bytes);
    let ref_hex = hex::encode(external_ref);
    if state.used_external_refs.contains(&ref_hex) {
        anyhow::bail!("duplicate external_ref (already minted)");
    }

    let key_path = data_dir
        .join("keys")
        .join(format!("validator-{validator_index}.json"));
    let key_file: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(&key_path)?)?;
    let minter = Keypair::from_file(&key_file).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let minter_sid = short_id(&minter.public_key());
    let nonce = state
        .account(&minter_sid)
        .map(|a| a.nonce)
        .unwrap_or(0);

    let body = TxBody::Mint {
        nonce,
        to: to_sid,
        amount,
        external_ref,
        to_pubkey: Some(to_pk),
    };
    let tx = Tx::sign(body, &minter).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    tx.verify().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Preferred path: submit to live gossip so all validators share state.
    let peer = if peer.is_empty() {
        std::env::var("MESH_MINT_PEER").unwrap_or_default()
    } else {
        peer.to_string()
    };
    if !peer.is_empty() && !offline {
        let tx_path = data_dir.join("last_mint.json");
        fs::write(&tx_path, serde_json::to_string_pretty(&tx)?)?;
        run::submit_tx_file(&tx_path, &peer)?;
        println!(
            "submitted mint {amount} base units → {} via {peer}",
            short_id_hex(&to_sid)
        );
        println!("txid: {}", tx.txid_hex());
        println!("await finality on live validators (poll scanner / chain_state)");
        return Ok(());
    }

    if !offline && std::env::var("MESH_ALLOW_OFFLINE_MINT").ok().as_deref() != Some("1") {
        anyhow::bail!(
            "refusing offline mint on multi-validator hosts. Pass --peer 127.0.0.1:9100 \
             or --offline with MESH_ALLOW_OFFLINE_MINT=1 (lab only)"
        );
    }

    eprintln!("WARN: offline mint finality (lab only) — can diverge multi-process nodes");
    let n = state.validators.len();
    if state.applied.is_empty() {
        let idx = leader_index(0, n);
        let vkey_path = data_dir.join("keys").join(format!("validator-{idx}.json"));
        let vfile: meshchain_proto::crypto::KeypairFile =
            serde_json::from_str(&fs::read_to_string(&vkey_path)?)?;
        let vk = Keypair::from_file(&vfile).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let gblock = produce_block(&state, &vk, idx as u8, now_secs(), vec![])
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        state
            .apply_block(&gblock)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }

    let next = state.height + 1;
    let idx = leader_index(next, n);
    let vkey_path = data_dir.join("keys").join(format!("validator-{idx}.json"));
    let vfile: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(&vkey_path)?)?;
    let producer = Keypair::from_file(&vfile).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let block = produce_block(&state, &producer, idx as u8, now_secs(), vec![tx])
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut finality = FinalityTracker::new();
    let hash = block.hash_hex();
    for i in 0..n {
        let p = data_dir.join("keys").join(format!("validator-{i}.json"));
        let f: meshchain_proto::crypto::KeypairFile =
            serde_json::from_str(&fs::read_to_string(&p)?)?;
        let k = Keypair::from_file(&f).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        finality.ack(&hash, k.public_key());
    }
    if !finality.is_final(&hash, n) {
        anyhow::bail!("not final");
    }
    state
        .apply_block(&block)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state
        .save_json(&state_path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let bal = state.balance_of(&to_sid);
    println!("minted {amount} base units tMESH → {}", short_id_hex(&to_sid));
    println!("recipient balance: {bal} ({:.6} tMESH)", bal as f64 / 1_000_000.0);
    println!("height: {}", state.height);
    println!("chain_id: {}", state.chain_id);
    Ok(())
}

fn burn_for_withdraw(
    data_dir: &std::path::Path,
    wallet_path: &std::path::Path,
    cold_path: &std::path::Path,
    amount: u64,
    dest_sol: &str,
    asset_id: u32,
) -> Result<()> {
    use crate::consensus::{leader_index, produce_block, FinalityTracker};
    use meshchain_ledger::state::ChainState;
    use meshchain_proto::address::{mesh_name, short_id, short_id_hex};
    use meshchain_proto::privacy::redeem_hint;
    use meshchain_proto::pq::PqKeypair;
    use meshchain_proto::tx::{Tx, TxBody};

    let _genesis: GenesisConfig = serde_json::from_str(&fs::read_to_string(
        data_dir.join("genesis.json"),
    )?)?;
    let state_path = data_dir.join("chain_state.json");
    let mut state = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let wfile: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(wallet_path)?)?;
    let wallet = Keypair::from_file(&wfile).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let cfile: meshchain_proto::pq::PqKeypairFile =
        serde_json::from_str(&fs::read_to_string(cold_path)?)?;
    let cold = PqKeypair::from_file(&cfile).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let from = short_id(&wallet.public_key());
    let acc = state
        .account(&from)
        .ok_or_else(|| anyhow::anyhow!("wallet not on chain"))?
        .clone();
    if acc.balance < amount {
        anyhow::bail!("insufficient balance");
    }

    let hint = redeem_hint(b"sol", dest_sol.as_bytes());
    let body = TxBody::Burn {
        nonce: acc.nonce,
        from,
        amount,
        redeem_hint: hint,
        asset_id,
    };
    let tx = Tx::sign_with_pq(body, &wallet, &cold).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let burn_txid = tx.txid();
    let burn_hex = hex::encode(burn_txid);

    let n = state.validators.len();
    let next = state.height + 1;
    let idx = leader_index(next, n);
    let vkey_path = data_dir.join("keys").join(format!("validator-{idx}.json"));
    let vfile: meshchain_proto::crypto::KeypairFile =
        serde_json::from_str(&fs::read_to_string(&vkey_path)?)?;
    let producer = Keypair::from_file(&vfile).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let block = produce_block(&state, &producer, idx as u8, now_secs(), vec![tx])
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut finality = FinalityTracker::new();
    let hash = block.hash_hex();
    for i in 0..n {
        let p = data_dir.join("keys").join(format!("validator-{i}.json"));
        let f: meshchain_proto::crypto::KeypairFile =
            serde_json::from_str(&fs::read_to_string(&p)?)?;
        let k = Keypair::from_file(&f).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        finality.ack(&hash, k.public_key());
    }
    state
        .apply_block(&block)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state
        .save_json(&state_path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let out = serde_json::json!({
        "burn_txid_hex": burn_hex,
        "amount": amount,
        "mesh_height": state.height,
        "mesh_short_id_hex": short_id_hex(&from),
        "mesh_name": mesh_name(&from),
        "dest_sol": dest_sol,
        "asset_id": asset_id,
        "balance_after": state.balance_of(&from),
    });
    let out_path = data_dir.join("last_burn.json");
    fs::write(&out_path, serde_json::to_string_pretty(&out)?)?;
    println!("burn finalized height={}", state.height);
    println!("burn_txid: {burn_hex}");
    println!("mesh name: {}", mesh_name(&from));
    println!("balance after: {} tMESH", state.balance_of(&from) as f64 / 1e6);
    println!("wrote {}", out_path.display());
    Ok(())
}
