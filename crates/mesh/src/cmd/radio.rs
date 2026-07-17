//! Send / radio / air-path commands:
//! send, air-submit, cold-demo, how-cold-works.

use anyhow::{bail, Context, Result};
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{mesh_name, parse_recipient, short_id};
use meshchain_proto::tx::{Tx, TxBody};
use meshchain_transport::{fragment_bytes, session_id_from_hash};
use std::fs;
use std::path::{Path, PathBuf};

use crate::helpers::{
    best_chain_state_path, default_submit_peer, fmt_mesh, load_cold, load_wallet,
    parse_mesh_amount, promote_v0_snapshot, refresh_after_submit, run_external_node,
    submit_tx_to_peer, wallet_path,
};

// ── send ──────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn cmd_send(
    dir: &Path,
    to: &str,
    amount: &str,
    wallet: &str,
    cold: &str,
    fee: &str,
    out: Option<PathBuf>,
    submit: Option<String>,
    wait: bool,
    air: bool,
    relay: &str,
) -> Result<()> {
    promote_v0_snapshot(dir);
    let state_path = best_chain_state_path(dir);
    if !state_path.exists() {
        bail!("No network yet. Run: mesh setup && mesh demo");
    }
    let wpath = wallet_path(dir, wallet);
    let kp = load_wallet(&wpath)?;
    let st = ChainState::load_json(&state_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let sid = short_id(&kp.public_key());
    let peer_hint = default_submit_peer(dir);
    let acc = st.account(&sid).with_context(|| {
        format!(
            "This wallet is not on the network yet. Run:\n  mesh register --wallet {wallet} --submit {peer_hint}"
        )
    })?;
    let units = parse_mesh_amount(amount)?;
    let fee_units = parse_mesh_amount(fee)?;
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
    let to_sid = parse_recipient(to).map_err(|e| anyhow::anyhow!(e))?;
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
        let cpath = wallet_path(dir, cold);
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

    if air {
        let target = if !relay.is_empty() {
            relay.to_string()
        } else if let Some(ref p) = submit {
            p.clone()
        } else {
            std::env::var("MESH_RADIO_RELAY").unwrap_or_else(|_| "127.0.0.1:9199".into())
        };
        println!("Air path → {target} (MC frame + mempool inject)");
        run_external_node(&[
            "submit-tx",
            "--tx",
            out_path.to_str().unwrap_or("last_payment.json"),
            "--peer",
            &target,
            "--air",
        ])?;
        if wait {
            println!("Waiting for inclusion…");
            let seed = submit.unwrap_or_else(|| default_submit_peer(dir));
            refresh_after_submit(dir, &seed);
            if let Ok(st2) = ChainState::load_json(&best_chain_state_path(dir)) {
                println!(
                    "Network now block #{} · your balance {} MESH",
                    st2.height,
                    fmt_mesh(st2.balance_of(&sid))
                );
            }
        }
    } else if let Some(peer) = submit {
        submit_tx_to_peer(&out_path, &peer)?;
        println!("Submitted to {peer}");
        if wait {
            println!("Waiting for inclusion…");
            refresh_after_submit(dir, &peer);
            if let Ok(st2) = ChainState::load_json(&best_chain_state_path(dir)) {
                println!(
                    "Network now block #{} · your balance {} MESH",
                    st2.height,
                    fmt_mesh(st2.balance_of(&sid))
                );
            }
        }
    } else {
        let peer = default_submit_peer(dir);
        println!();
        println!("Signed only. Submit with:");
        println!("  mesh send {to} {amount} --wallet {wallet} --submit {peer}");
        println!("  # or Meshtastic air path:");
        println!("  mesh send {to} {amount} --wallet {wallet} --air --relay 127.0.0.1:9199");
    }
    Ok(())
}

// ── air-submit ────────────────────────────────────────────────────────────────

pub fn cmd_air_submit(dir: &Path, tx: &str, peer: &str, relay: &str) -> Result<()> {
    let tx_path = if tx.is_empty() {
        dir.join("last_payment.json")
    } else {
        let p = PathBuf::from(tx);
        if p.is_absolute() || tx.contains('/') {
            p
        } else {
            dir.join(tx)
        }
    };
    if !tx_path.exists() {
        bail!(
            "No signed tx at {}. First: mesh send …  then air-submit",
            tx_path.display()
        );
    }
    let target = if !relay.is_empty() {
        relay.to_string()
    } else if !peer.is_empty() {
        peer.to_string()
    } else {
        std::env::var("MESH_RADIO_RELAY").unwrap_or_else(|_| "127.0.0.1:9199".into())
    };
    println!("Air-submit {} → {target}", tx_path.display());
    run_external_node(&[
        "submit-tx",
        "--tx",
        tx_path.to_str().unwrap_or("last_payment.json"),
        "--peer",
        &target,
        "--air",
    ])?;
    println!(
        "Sent MC frame path (tx_air + MCHEX). Needs local validator + optional mesh_radio_relay."
    );
    println!("Docs: docs/MESHTASTIC.md");
    Ok(())
}

// ── cold-demo ─────────────────────────────────────────────────────────────────

pub fn cmd_cold_demo(dir: &Path) -> Result<()> {
    println!("Cold storage radio demo…");
    run_external_node(&[
        "pq-cold-demo",
        "--data-dir",
        dir.to_str().unwrap_or("./data"),
    ])?;
    Ok(())
}

// ── how-cold-works ────────────────────────────────────────────────────────────

pub fn cmd_how_cold_works() {
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
