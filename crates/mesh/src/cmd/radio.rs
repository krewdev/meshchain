//! Send / radio / air-path commands:
//! send, air-submit, cold-demo, how-cold-works.

use anyhow::{bail, Context, Result};
use meshchain_ledger::state::ChainState;
use meshchain_proto::address::{mesh_name, parse_recipient, short_id};
use meshchain_proto::tx::{Tx, TxBody};
use meshchain_transport::{
    decode_bal_reply, decode_frame, encode_bal_query, fragment_bytes, session_id_from_hash,
    BalQuery,
};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::helpers::{
    best_chain_state_path, default_submit_peer, fmt_mesh, load_cold, load_wallet,
    parse_mesh_amount, promote_v0_snapshot, refresh_after_submit, run_external_node,
    submit_tx_to_peer, wallet_path,
};

fn default_relay(relay: &str) -> String {
    if !relay.is_empty() {
        return relay.to_string();
    }
    std::env::var("MESH_RADIO_RELAY").unwrap_or_else(|_| "127.0.0.1:9199".into())
}

/// Ask a radio relay (or mock) for this short id's balance. One LoRa frame each way.
pub fn cmd_air_balance(dir: &Path, wallet: &str, relay: &str, name: &str) -> Result<()> {
    let sid = if !name.is_empty() {
        parse_recipient(name).map_err(|e| anyhow::anyhow!(e))?
    } else {
        let kp = load_wallet(&wallet_path(dir, wallet))?;
        short_id(&kp.public_key())
    };
    let target = default_relay(relay);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u32)
        .unwrap_or(1);
    let req_id = nanos.to_le_bytes();
    let frame = encode_bal_query(&BalQuery {
        short_id: sid,
        req_id,
    })
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    println!("Radio balance query");
    println!("  Name:   {}", mesh_name(&sid));
    println!("  Relay:  {target}");
    println!("  Frame:  {} B (MC BalQuery)", frame.len());

    let mut stream = TcpStream::connect(&target).with_context(|| {
        format!(
            "Could not reach radio relay {target}.\n  Start one: ./scripts/start_radio_relay.sh\n  Or: python3 tools/mesh_radio_relay.py --mock --tcp 127.0.0.1:9100 --data-dir ./data"
        )
    })?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(8)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    writeln!(stream, "MCHEX {}", hex::encode(&frame))?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let want = hex::encode(req_id);
    let mut line = String::new();
    let deadline = SystemTime::now() + Duration::from_secs(8);
    loop {
        if SystemTime::now() > deadline {
            bail!(
                "No radio reply in 8s. Is the relay up and does it have chain_state?\n  python3 tools/mesh_radio_relay.py --mock --data-dir ./data --tcp 127.0.0.1:9100"
            );
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => bail!("Relay closed the connection before a balance reply."),
            Ok(_) => {}
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(reply) = parse_bal_reply_line(t, &want, &sid) {
            println!("Balance:   {} MESH", fmt_mesh(reply.balance));
            println!("Sends:     {} completed from this wallet", reply.nonce);
            println!("Network:   block #{} (via radio)", reply.height);
            if !reply.found {
                println!("On-chain:  NO — this name is not in the relay's chain state yet.");
            } else {
                println!("Path:      LoRa / radio relay (no internet used)");
            }
            return Ok(());
        }
    }
}

struct AirBal {
    balance: u64,
    nonce: u64,
    height: u64,
    found: bool,
}

fn parse_bal_reply_line(line: &str, want_req: &str, sid: &[u8; 8]) -> Option<AirBal> {
    let upper = line.to_ascii_uppercase();
    if upper.starts_with("MCHEX ") {
        let hx = line.split_whitespace().nth(1)?;
        let raw = hex::decode(hx).ok()?;
        let frame = decode_frame(&raw).ok()?;
        let r = decode_bal_reply(&frame).ok()?;
        if &r.short_id == sid && hex::encode(r.req_id) == want_req {
            return Some(AirBal {
                balance: r.balance,
                nonce: r.nonce,
                height: r.height,
                found: r.found,
            });
        }
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(|x| x.as_str()) != Some("bal_reply") {
        return None;
    }
    let req = v.get("req_id_hex").and_then(|x| x.as_str()).unwrap_or("");
    let got_sid = v.get("short_id_hex").and_then(|x| x.as_str()).unwrap_or("");
    if req != want_req || got_sid != hex::encode(sid) {
        return None;
    }
    Some(AirBal {
        balance: v.get("balance").and_then(|x| x.as_u64()).unwrap_or(0),
        nonce: v.get("nonce").and_then(|x| x.as_u64()).unwrap_or(0),
        height: v.get("height").and_then(|x| x.as_u64()).unwrap_or(0),
        found: v.get("found").and_then(|x| x.as_bool()).unwrap_or(false),
    })
}

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

// ── radio bridge ──────────────────────────────────────────────────────────────

pub fn cmd_radio_bridge(
    dir: &Path,
    port: Option<String>,
    delay_ms: Option<u64>,
    portnum: Option<u32>,
) -> Result<()> {
    let cfg = crate::config::MeshConfig::load_or_default(dir);
    let target_port = port.or(cfg.radio_port).context(
        "No radio port specified. Please provide --port or set default with: mesh config set --port <port>"
    )?;
    let target_delay = delay_ms.unwrap_or(cfg.tx_delay_ms);
    let target_portnum = portnum.unwrap_or(cfg.portnum);

    println!("Starting stdio Meshtastic radio bridge...");
    println!("  Port:     {target_port}");
    println!("  Delay:    {target_delay} ms");
    println!("  PortNum:  {target_portnum}");

    let mut cmd = std::process::Command::new("python3");
    cmd.arg("tools/meshtastic_bridge.py");
    if target_port == "mock" {
        cmd.arg("--mock");
    } else {
        cmd.arg("--port").arg(&target_port);
    }
    cmd.arg("--tx-delay-ms").arg(target_delay.to_string());
    cmd.arg("--portnum").arg(target_portnum.to_string());

    let mut child = cmd
        .spawn()
        .with_context(|| "Failed to spawn meshtastic_bridge.py")?;
    let status = child.wait()?;
    if !status.success() {
        bail!(
            "Radio bridge process exited with error status: {:?}",
            status.code()
        );
    }
    Ok(())
}

// ── radio info ────────────────────────────────────────────────────────────────

pub fn cmd_radio_info(dir: &Path, port: Option<String>) -> Result<()> {
    let cfg = crate::config::MeshConfig::load_or_default(dir);
    let target_port = port
        .or(cfg.radio_port)
        .unwrap_or_else(|| "mock".to_string());

    println!("Meshtastic Device Info");
    println!("──────────────────────");
    println!("Configured Port:  {}", target_port);

    if target_port == "mock" {
        println!("Status:           CONNECTED (Mock mode)");
        println!("Node ID:          !abc12345");
        println!("Hardware:         MockRadio v2");
        println!("Battery Level:    98%");
        println!("Air Queue:        0 packets");
        println!(
            "Channels Config:  MeshChain-Testnet-1 (PortNum: {})",
            cfg.portnum
        );
        return Ok(());
    }

    // Try to run inline python to query info
    let py_code = format!(
        r#"import sys
try:
    import meshtastic
    import meshtastic.serial_interface
except ImportError:
    print("MISSING_LIB")
    sys.exit(0)

try:
    iface = meshtastic.serial_interface.SerialInterface(devPath={:?})
    print("OK")
    print(f"NodeId: !{{iface.myInfo.my_node_num:08x}}")
    print(f"HwModel: {{getattr(iface.myInfo, 'hw_model', 'Unknown')}}")
    print(f"NodesCount: {{len(iface.nodes) if iface.nodes else 0}}")
    iface.close()
except Exception as e:
    print(f"ERROR: {{e}}")
"#,
        target_port
    );

    let output = std::process::Command::new("python3")
        .arg("-c")
        .arg(&py_code)
        .output()?;
    let out_str = String::from_utf8_lossy(&output.stdout);

    if out_str.contains("MISSING_LIB") {
        println!("Status:           DISCONNECTED (meshtastic Python package not installed)");
        println!("\nTo enable full hardware queries over Serial, install the meshtastic library:");
        println!("  pip install meshtastic");
        println!("\nOr run with mock mode:");
        println!("  mesh radio info --port mock");
    } else if out_str.contains("OK") {
        println!("Status:           CONNECTED (Hardware Serial)");
        for line in out_str.lines() {
            if let Some(rest) = line.strip_prefix("NodeId:") {
                println!("Node ID:          {}", rest.trim());
            } else if let Some(rest) = line.strip_prefix("HwModel:") {
                println!("Hardware:         {}", rest.trim());
            } else if let Some(rest) = line.strip_prefix("NodesCount:") {
                println!("Mesh Node Count:  {}", rest.trim());
            }
        }
    } else {
        println!("Status:           DISCONNECTED");
        let err_msg = out_str
            .lines()
            .find(|l| l.starts_with("ERROR:"))
            .map(|l| &l[6..])
            .unwrap_or("Connection failed");
        println!("Error:           {}", err_msg.trim());
    }

    Ok(())
}
