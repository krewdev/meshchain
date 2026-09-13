# lib.rs Path A splice

`relay.rs` is on the branch. `lib.rs` still needs the inserts below, or run:

```
python3 programs-mesh-bridge/scripts/apply_lib_rs_splice.py
cd programs-mesh-bridge && anchor build
```

## 1. After `declare_id!`

```rust
pub mod relay;
```

## 2. Before the `#[program]` module closes (ahead of `fn mul_bps`)

```rust
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
```

## 3. `count_attestor_signers` → `pub(crate) fn`

## 4. Append to `BridgeError`

`EmissionsDark`, `InvalidValidatorIndex`, `NotAttestor`, `NotSettleAttestor`, `FeeAlreadyClaimed`.

Do not `anchor deploy` a new id. Upgrade `CBRQcjk5…` on **devnet** only.
