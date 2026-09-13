#!/usr/bin/env python3
"""Insert Path A wrappers into programs-mesh-bridge src/lib.rs.

Run from programs-mesh-bridge/ or repo root:
  python3 scripts/apply_lib_rs_splice.py

Idempotent. Does not touch withdraw_hybrid_*. Does not flip emissions.
"""
from __future__ import annotations

from pathlib import Path

CANDIDATES = [
    Path("programs/programs-mesh-bridge/src/lib.rs"),
    Path("programs-mesh-bridge/programs/programs-mesh-bridge/src/lib.rs"),
]

MOD = 'pub mod relay;'

WRAPPERS = '''
    pub fn init_relay_config(
        ctx: Context<crate::relay::InitRelayConfig>,
        relay_bps: u16,
        emissions_enabled: bool,
    ) -> Result<()> {
        crate::relay::init_relay_config(ctx, relay_bps, emissions_enabled)
    }

    pub fn set_emissions_enabled(
        ctx: Context<crate::relay::AuthRelay>,
        enabled: bool,
        relay_mint: Pubkey,
    ) -> Result<()> {
        crate::relay::set_emissions_enabled(ctx, enabled, relay_mint)
    }

    pub fn register_node(
        ctx: Context<crate::relay::RegisterNode>,
        mesh_short_id: [u8; 8],
    ) -> Result<()> {
        crate::relay::register_node(ctx, mesh_short_id)
    }

    pub fn post_air_ack(
        ctx: Context<crate::relay::PostAirAck>,
        height: u64,
        block_hash: [u8; 32],
        validator_index: u8,
    ) -> Result<()> {
        crate::relay::post_air_ack(ctx, height, block_hash, validator_index)
    }

    pub fn open_settle_credit(
        ctx: Context<crate::relay::OpenSettleCredit>,
        burn_txid: [u8; 32],
    ) -> Result<()> {
        crate::relay::open_settle_credit(ctx, burn_txid)
    }

    pub fn claim_settle_fee(ctx: Context<crate::relay::ClaimSettleFee>, index: u8) -> Result<()> {
        crate::relay::claim_settle_fee(ctx, index)
    }

    pub fn claim_settle(ctx: Context<crate::relay::ClaimSettle>, index: u8) -> Result<()> {
        crate::relay::claim_settle(ctx, index)
    }
'''

ERRORS = '''
    #[msg("RELAY emissions are dark \u2014 Path A. Flip only after MeshBridge is mainnet.")]
    EmissionsDark,
    #[msg("validator_index out of attestor set")]
    InvalidValidatorIndex,
    #[msg("signer is not a registered attestor")]
    NotAttestor,
    #[msg("signer is not listed on this settle credit")]
    NotSettleAttestor,
    #[msg("settle fee already claimed for this index")]
    FeeAlreadyClaimed,
'''


def find_lib() -> Path:
    for p in CANDIDATES:
        if p.exists():
            return p
    raise SystemExit("lib.rs not found; run from repo root or programs-mesh-bridge/")


def main() -> None:
    path = find_lib()
    text = path.read_text()
    changed = False

    if MOD not in text:
        needle = 'declare_id!("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");'
        if needle not in text:
            raise SystemExit("declare_id! needle missing")
        text = text.replace(needle, needle + "\n\n" + MOD, 1)
        changed = True
        print("inserted pub mod relay")
    else:
        print("pub mod relay already present")

    if "fn init_relay_config(" not in text:
        mark = "\n}\n\nfn mul_bps"
        if mark not in text:
            raise SystemExit("cannot find program-module close before mul_bps")
        text = text.replace(mark, WRAPPERS + mark, 1)
        changed = True
        print("inserted seven Path A wrappers")
    else:
        print("wrappers already present")

    if "fn count_attestor_signers" in text and "pub(crate) fn count_attestor_signers" not in text:
        text = text.replace(
            "fn count_attestor_signers",
            "pub(crate) fn count_attestor_signers",
            1,
        )
        changed = True
        print("made count_attestor_signers pub(crate)")

    if "EmissionsDark" not in text:
        mark = "    #[msg(\"Mint mismatch for this deposit\")]\n    MintMismatch,\n}"
        if mark not in text:
            raise SystemExit("cannot find BridgeError tail")
        text = text.replace(
            mark,
            "    #[msg(\"Mint mismatch for this deposit\")]\n    MintMismatch," + ERRORS + "}",
            1,
        )
        changed = True
        print("appended Path A BridgeError variants")
    else:
        print("BridgeError Path A variants already present")

    if changed:
        path.write_text(text)
        print("wrote", path)
    else:
        print("no changes")
    print("next: cd programs-mesh-bridge && anchor build")


if __name__ == "__main__":
    main()
