//! Wallet commands: new-wallet, address, balance, register, new-cold-key.

use anyhow::{bail, Result};
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{mesh_name, short_id, short_id_hex};
use meshchain_proto::crypto::Keypair;
use meshchain_proto::pq::PqKeypair;
use meshchain_proto::tx::{Tx, TxBody};
use std::fs;
use std::path::{Path, PathBuf};

use crate::helpers::{
    best_chain_state_path, default_submit_peer, fmt_mesh, keys_dir, load_wallet,
    promote_v0_snapshot, refresh_after_submit, submit_tx_to_peer, sync_state_from_scanner,
    wallet_path, default_scanner_url,
};

// ── register helper (shared by new-wallet --publish and register cmd) ─────────

pub fn sign_register(dir: &Path, wallet: &str, out: Option<PathBuf>) -> Result<(PathBuf, Tx)> {
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

// ── new-wallet ────────────────────────────────────────────────────────────────

pub fn cmd_new_wallet(dir: &Path, name: &str, publish: bool, submit: &str) -> Result<()> {
    fs::create_dir_all(keys_dir(dir))?;
    let path = wallet_path(dir, name);
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

    // Attempt to hit the local faucet if it is running, to auto-mint on chain
    let pubkey_hex = hex::encode(kp.public_key());
    println!("Attempting to auto-mint 100 tMESH via local faucet...");
    let payload = serde_json::json!({
        "mesh_name": tag,
        "public_key_hex": pubkey_hex
    });
    let payload_str = payload.to_string();

    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg("-X")
        .arg("POST")
        .arg("http://127.0.0.1:8787/drip")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-d")
        .arg(&payload_str)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            if let Ok(res_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if res_val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    println!("✅ Successfully auto-minted 100 tMESH! Wallet registered on-chain.");
                } else if let Some(err) = res_val.get("error").and_then(|v| v.as_str()) {
                    println!("ℹ️ Faucet responded: {}", err);
                } else {
                    println!("ℹ️ Faucet contacted, but registration status is unclear.");
                }
            } else {
                println!("ℹ️ Faucet contacted, but response could not be parsed.");
            }
        }
        _ => {
            println!("ℹ️ Local faucet not running/reachable. Run `mesh testnet-setup` or start the host to activate the faucet.");
        }
    }

    println!();
    let peer = if submit.is_empty() {
        default_submit_peer(dir)
    } else {
        submit.to_string()
    };
    if publish {
        let name_only = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(name);
        match sign_register(dir, name_only, None) {
            Ok((reg_path, tx)) => {
                println!("Register signed → {}", reg_path.display());
                println!("  Id: {}", tx.txid_hex());
                if let Err(e) = submit_tx_to_peer(&reg_path, &peer) {
                    eprintln!("WARN: submit failed: {e}");
                    eprintln!("  Retry: mesh register --wallet {name_only} --submit {peer}");
                } else {
                    println!("Submitted Register to {peer}");
                    refresh_after_submit(dir, &peer);
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
    Ok(())
}

// ── new-cold-key ──────────────────────────────────────────────────────────────

pub fn cmd_new_cold_key(dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(keys_dir(dir))?;
    let path = wallet_path(dir, name);
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
    Ok(())
}

// ── address ───────────────────────────────────────────────────────────────────

pub fn cmd_address(dir: &Path, wallet: &str) -> Result<()> {
    let path = wallet_path(dir, wallet);
    let kp = load_wallet(&path)?;
    let sid = short_id(&kp.public_key());
    println!("Wallet:    {}", path.display());
    println!("Mesh name: {}", mesh_name(&sid));
    println!("Hex id:    {}", short_id_hex(&sid));
    println!("(Share your mesh name — like M4K7X-J9P2Q-R3W — so people can pay you.)");
    Ok(())
}

// ── balance ───────────────────────────────────────────────────────────────────

pub fn cmd_balance(dir: &Path, wallet: &str) -> Result<()> {
    let path = wallet_path(dir, wallet);
    promote_v0_snapshot(dir);
    // Prefer live scanner/seed tip so faucet-mint lag is less confusing.
    if let Some(sc) = default_scanner_url(dir) {
        let _ = sync_state_from_scanner(dir, &sc);
    }
    let state_path = best_chain_state_path(dir);
    if !state_path.exists() {
        bail!(
            "No network state yet. Run:\n  mesh join-public\n  # or mesh testnet-setup + demo"
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
        println!("On-chain:  NO — run: mesh register --wallet {wallet} --submit <seed:9100>");
    }
    if cold {
        println!("Cold key:  locked to this account (large sends use it)");
    } else {
        println!(
            "Cold key:  not bound yet (needed for sends ≥ {} MESH)",
            fmt_mesh(st.pq_required_above)
        );
    }
    Ok(())
}

// ── register ──────────────────────────────────────────────────────────────────

pub fn cmd_register(
    dir: &Path,
    wallet: &str,
    out: Option<PathBuf>,
    submit: Option<String>,
    no_submit: bool,
) -> Result<()> {
    let (out_path, tx) = sign_register(dir, wallet, out)?;
    let sid = short_id(&tx.signer);
    println!("Register signed.");
    println!("  Mesh name: {}", mesh_name(&sid));
    println!("  Id:        {}", tx.txid_hex());
    println!("  File:      {}", out_path.display());
    if no_submit {
        let peer = default_submit_peer(dir);
        println!();
        println!("Signed only. Submit with:");
        println!("  mesh register --wallet {wallet} --submit {peer}");
    } else {
        let peer = submit.unwrap_or_else(|| default_submit_peer(dir));
        submit_tx_to_peer(&out_path, &peer)?;
        println!("Submitted to {peer}");
        println!("Account will appear after the next block (often a few seconds).");
        refresh_after_submit(dir, &peer);
    }
    Ok(())
}

// ── validator-keygen ──────────────────────────────────────────────────────────

pub fn cmd_validator_keygen(dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(keys_dir(dir))?;
    let path = wallet_path(dir, name);
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
    println!("Docs: docs/RUN_A_NODE.md · docs/MULTI_OPERATOR.md");
    println!("Template: testnet/operator_application.example.json");
    Ok(())
}
