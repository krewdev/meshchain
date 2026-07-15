//! Info / diagnostic commands: security, demo.

use anyhow::Result;
use std::path::Path;

use crate::helpers::run_external_node;

// ── security ──────────────────────────────────────────────────────────────────

pub fn cmd_security() {
    println!(
        r#"
Security & privacy posture
──────────────────────────
We aim for maximum rigor. We do NOT claim "unhackable" or "perfect anonymity."

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

// ── demo ──────────────────────────────────────────────────────────────────────

pub fn cmd_demo(dir: &Path, transfers: u32) -> Result<()> {
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
    Ok(())
}
