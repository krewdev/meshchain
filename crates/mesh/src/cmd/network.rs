//! Network / node management commands:
//! setup, testnet-setup, testnet-info, testnet-attestors,
//! join-public, sync-state, faucet-drip, status,
//! validator, observer, genesis-extend/add.

use anyhow::{bail, Context, Result};
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{mesh_name, short_id};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::helpers::{
    best_chain_state_path, default_faucet_url, default_scanner_url, default_submit_peer, fmt_mesh,
    http_post_json, keys_dir, load_wallet, promote_v0_snapshot, run_external_node,
    sync_state_from_scanner, wallet_path,
};

// ── install_public_artifacts (used by join-public + testnet-setup) ────────────

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

// ── testnet-setup ─────────────────────────────────────────────────────────────

pub fn cmd_testnet_setup(dir: &Path, validators: u8) -> Result<()> {
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
    Ok(())
}

// ── testnet-info ──────────────────────────────────────────────────────────────

pub fn cmd_testnet_info(dir: &Path) -> Result<()> {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║         MeshChain PUBLIC TESTNET                 ║");
    println!("║         meshchain-testnet-1                      ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();
    println!("Status:     active (public seed live)");
    println!("Token:      tMESH — NO CASH VALUE");
    println!("chain_id:   meshchain-testnet-1");
    println!("Channel:    MeshChain-Testnet-1  (private Meshtastic)");
    println!("Solana:     devnet only for bridge experiments");
    println!("Docs:       https://meshchain-sigma.vercel.app/docs/?doc=TESTNET");
    println!("Run a node: https://meshchain-sigma.vercel.app/docs/?doc=RUN_A_NODE");
    println!("Params:     https://meshchain-sigma.vercel.app/testnet/network.json");
    println!("Seeds:      https://meshchain-sigma.vercel.app/testnet/seeds.json");
    println!("Submit:     {}", default_submit_peer(dir));
    if let Some(s) = default_scanner_url(dir) {
        println!("Scanner:    {s}");
    }
    if let Some(f) = default_faucet_url(dir) {
        println!("Faucet:     {f}");
    }
    println!();
    println!("Join:       mesh join-public");
    println!("Wallet:     mesh new-wallet --name me.json --publish");
    println!("Drip:       mesh faucet-drip --wallet me.json");
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

// ── join-public ───────────────────────────────────────────────────────────────

pub fn cmd_join_public(dir: &Path, scanner: Option<String>) -> Result<()> {
    println!("Joining MeshChain PUBLIC testnet profile…");
    let (peer, scanner_from_seeds) = install_public_artifacts(dir)?;
    let scanner = scanner.or(scanner_from_seeds);
    if let Some(ref sc) = scanner {
        match sync_state_from_scanner(dir, sc) {
            Ok(st) => {
                println!("Synced live state height={}", st.height);
            }
            Err(e) => {
                eprintln!("WARN: sync-state skipped: {e}");
                eprintln!("  Retry: mesh sync-state --scanner {sc}");
            }
        }
    }
    let faucet = default_faucet_url(dir);
    println!();
    println!("Done. Public seed peer: {peer}");
    println!("Next:");
    println!("  mesh new-wallet --name me.json --publish");
    println!("  mesh faucet-drip --wallet me.json");
    println!("  mesh balance --wallet me.json");
    if let Some(sc) = &scanner {
        println!("  Scanner: {sc}");
    }
    if let Some(f) = faucet {
        println!("  Faucet:  {f}");
    }
    println!("  Docs: docs/RUN_A_NODE.md");
    Ok(())
}

// ── sync-state ────────────────────────────────────────────────────────────────

pub fn cmd_sync_state(dir: &Path, scanner: &str) -> Result<()> {
    let sc = if scanner.is_empty() {
        default_scanner_url(dir).context(
            "No scanner URL. Pass --scanner http://HOST:8788 or set MESH_SCANNER / join-public first.",
        )?
    } else {
        scanner.to_string()
    };
    let st = sync_state_from_scanner(dir, &sc)?;
    println!(
        "Network {} · block #{} · supply {}",
        st.chain_id,
        st.height,
        fmt_mesh(st.total_supply)
    );
    Ok(())
}

// ── faucet-drip ───────────────────────────────────────────────────────────────

pub fn cmd_faucet_drip(dir: &Path, wallet: &str, faucet: &str) -> Result<()> {
    let wpath = wallet_path(dir, wallet);
    let kp = load_wallet(&wpath)?;
    let pub_hex = hex::encode(kp.public_key());
    let sid = short_id(&kp.public_key());
    let name = mesh_name(&sid);
    let faucet_base = if faucet.is_empty() {
        default_faucet_url(dir).context(
            "No faucet URL. Pass --faucet https://… or set MESH_FAUCET / join-public first.",
        )?
    } else {
        faucet.to_string()
    };
    let faucet_base = faucet_base.trim_end_matches('/');
    let drip_url = if faucet_base.ends_with("/drip") {
        faucet_base.to_string()
    } else {
        format!("{faucet_base}/drip")
    };
    let body = serde_json::json!({
        "mesh_name": name,
        "public_key_hex": pub_hex,
    });
    println!("Requesting faucet drip for {name} …");
    println!("  POST {drip_url}");
    let resp = http_post_json(&drip_url, &serde_json::to_string(&body)?)?;
    println!("{resp}");
    if let Some(sc) = default_scanner_url(dir) {
        thread::sleep(Duration::from_secs(3));
        let _ = sync_state_from_scanner(dir, &sc);
        if let Ok(st) = ChainState::load_json(&dir.join("chain_state.json")) {
            println!(
                "Balance now: {} tMESH (block #{})",
                fmt_mesh(st.balance_of(&sid)),
                st.height
            );
        }
    }
    Ok(())
}

// ── setup (local dev) ─────────────────────────────────────────────────────────

pub fn cmd_setup(dir: &Path, validators: u8) -> Result<()> {
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
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

pub fn cmd_status(dir: &Path) -> Result<()> {
    promote_v0_snapshot(dir);
    let state_path = best_chain_state_path(dir);
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
    Ok(())
}

// ── validator ─────────────────────────────────────────────────────────────────

pub fn cmd_validator(dir: &Path, index: u8, listen: &str, peers: &[String]) -> Result<()> {
    println!("Starting testnet validator index={index} listen={listen}");
    println!("(Must match shared genesis.validators[{index}])");
    let mut args = vec![
        "run".to_string(),
        "--data-dir".into(),
        dir.to_str().unwrap_or("./data").into(),
        "--validator-index".into(),
        index.to_string(),
        "--listen".into(),
        listen.to_string(),
    ];
    for p in peers {
        args.push("--peer".into());
        args.push(p.clone());
    }
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_external_node(&args_ref)?;
    Ok(())
}

// ── observer ──────────────────────────────────────────────────────────────────

pub fn cmd_observer(dir: &Path, listen: &str, peers: &[String]) -> Result<()> {
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
        listen.to_string(),
    ];
    for p in peers {
        args.push("--peer".into());
        args.push(p.clone());
    }
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_external_node(&args_ref)?;
    Ok(())
}

// ── genesis-extend / genesis-add ─────────────────────────────────────────────

pub fn cmd_genesis_modify(genesis: &Path, add: &[String], out: &Path) -> Result<()> {
    if add.is_empty() {
        bail!("provide at least one --add <public_hex>");
    }
    let raw = fs::read_to_string(genesis).with_context(|| format!("read {}", genesis.display()))?;
    let mut g: serde_json::Value = serde_json::from_str(&raw).context("invalid genesis JSON")?;
    let vals = g
        .get_mut("validators")
        .and_then(|v| v.as_array_mut())
        .context("genesis.validators missing")?;
    let before = vals.len();
    for a in add {
        let h = a.trim().trim_start_matches("0x").to_lowercase();
        if h.len() != 64 || hex::decode(&h).map(|b| b.len() != 32).unwrap_or(true) {
            bail!("invalid public_hex (need 32 bytes hex): {a}");
        }
        let exists = vals.iter().any(|v| v.as_str() == Some(h.as_str()));
        if exists {
            println!("skip duplicate {h}");
            continue;
        }
        vals.push(serde_json::Value::String(h.clone()));
        println!("added index {}: {h}", vals.len() - 1);
    }
    let after = vals.len();
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string_pretty(&g)? + "\n")?;
    println!(
        "wrote {}  validators {before} → {after}  finality≥{}",
        out.display(),
        (2 * after).div_ceil(3)
    );
    println!(
        "Restack: ASSUME_YES=1 ./scripts/restack_public_seed.sh {}",
        out.display()
    );
    println!("Docs: docs/MULTI_OPERATOR.md");
    Ok(())
}

// ── testnet-attestors ─────────────────────────────────────────────────────────

pub fn cmd_testnet_attestors() -> Result<()> {
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
