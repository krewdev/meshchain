#!/usr/bin/env python3
"""Insert Path A wrappers into programs-mesh-bridge src/lib.rs.

Run from programs-mesh-bridge/ or repo root:
  python3 scripts/apply_lib_rs_splice.py

Idempotent. Does not touch withdraw_hybrid_*. Does not flip emissions.

Anchor's #[program] macro cannot take Context<crate::relay::Foo> — it looks
for __client_accounts_crate. Import the account structs at crate root instead.
"""
from __future__ import annotations

from pathlib import Path

CANDIDATES = [
    Path("programs/programs-mesh-bridge/src/lib.rs"),
    Path("programs-mesh-bridge/programs/programs-mesh-bridge/src/lib.rs"),
]

MOD = "pub mod relay;"
USE = (
    "use relay::{AuthRelay, ClaimSettle, ClaimSettleFee, InitRelayConfig,"
    " OpenSettleCredit, PostAirAck, RegisterNode};"
)

WRAPPERS = '''
    pub fn init_relay_config(
        ctx: Context<InitRelayConfig>,
        relay_bps: u16,
        emissions_enabled: bool,
    ) -> Result<()> {
        relay::init_relay_config(ctx, relay_bps, emissions_enabled)
    }

    pub fn set_emissions_enabled(
        ctx: Context<AuthRelay>,
        enabled: bool,
        relay_mint: Pubkey,
    ) -> Result<()> {
        relay::set_emissions_enabled(ctx, enabled, relay_mint)
    }

    pub fn register_node(
        ctx: Context<RegisterNode>,
        mesh_short_id: [u8; 8],
    ) -> Result<()> {
        relay::register_node(ctx, mesh_short_id)
    }

    pub fn post_air_ack(
        ctx: Context<PostAirAck>,
        height: u64,
        block_hash: [u8; 32],
        validator_index: u8,
    ) -> Result<()> {
        relay::post_air_ack(ctx, height, block_hash, validator_index)
    }

    pub fn open_settle_credit(
        ctx: Context<OpenSettleCredit>,
        burn_txid: [u8; 32],
    ) -> Result<()> {
        relay::open_settle_credit(ctx, burn_txid)
    }

    pub fn claim_settle_fee(ctx: Context<ClaimSettleFee>, index: u8) -> Result<()> {
        relay::claim_settle_fee(ctx, index)
    }

    pub fn claim_settle(ctx: Context<ClaimSettle>, index: u8) -> Result<()> {
        relay::claim_settle(ctx, index)
    }
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

    if "Context<crate::relay::" in text:
        text = text.replace("Context<crate::relay::", "Context<")
        text = text.replace("crate::relay::", "relay::")
        changed = True
        print("rewrote crate::relay:: Context paths (Anchor E0432)")

    if MOD not in text:
        needle = 'declare_id!("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");'
        if needle not in text:
            raise SystemExit("declare_id! needle missing")
        text = text.replace(needle, needle + "\n\n" + MOD + "\n" + USE, 1)
        changed = True
        print("inserted pub mod relay + account imports")
    else:
        print("pub mod relay already present")
        if USE not in text:
            text = text.replace(MOD, MOD + "\n" + USE, 1)
            changed = True
            print("inserted relay account imports")

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

    if changed:
        path.write_text(text)
        print("wrote", path)
    else:
        print("no changes")
    print("next: cd programs-mesh-bridge && anchor build")


if __name__ == "__main__":
    main()
