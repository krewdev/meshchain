# lib.rs splice (Path A)

File: `programs-mesh-bridge/programs/programs-mesh-bridge/src/lib.rs`
Program id stays `CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx`.

## 1. After the `use` block

```rust
pub mod relay;
```

## 2. Helper visibility

Change

```rust
fn count_attestor_signers(
```

to

```rust
pub(crate) fn count_attestor_signers(
```

## 3. Append these variants to `BridgeError` (end of enum, before `}`)

```rust
    #[msg("RELAY mint path is dark until mainnet vault + flag flip")]
    EmissionsDark,
    #[msg("validator_index out of range")]
    InvalidValidatorIndex,
    #[msg("signer is not a registered attestor")]
    NotAttestor,
    #[msg("signer is not a settle attestor at this index")]
    NotSettleAttestor,
    #[msg("this attestor already claimed the fee share")]
    FeeAlreadyClaimed,
```

## 4. Paste inside `#[program] pub mod programs_mesh_bridge`, after `withdraw_hybrid_spl`

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

    pub fn claim_settle_fee(
        ctx: Context<crate::relay::ClaimSettleFee>,
        index: u8,
    ) -> Result<()> {
        crate::relay::claim_settle_fee(ctx, index)
    }

    pub fn claim_settle(
        ctx: Context<crate::relay::ClaimSettle>,
        index: u8,
    ) -> Result<()> {
        crate::relay::claim_settle(ctx, index)
    }
```

Wrappers match the landed `src/relay.rs` (account names `validator` / `ack` / `node` / `score`).

Then:

```bash
cd programs-mesh-bridge
anchor build
```
