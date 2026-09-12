//! Path A RELAY. Splice with docs/INTEGRATION.md before `anchor build`.
//! Matches scripts/init_relay_devnet.ts and scripts/post_air_acks.ts.
//! claim_settle stays EmissionsDark until set_emissions_enabled.

use anchor_lang::prelude::*;
use crate::{
    BridgeConfig, BridgeError, SolVault, WithdrawRecord, CONFIG_SEED, MAX_ATTESTORS, VAULT_SEED,
    WITHDRAW_SEED,
};

pub const RELAY_CONFIG_SEED: &[u8] = b"mesh-relay-config";
pub const RELAY_NODE_SEED: &[u8] = b"mesh-relay-node";
pub const RELAY_ACK_SEED: &[u8] = b"mesh-relay-ack";
pub const RELAY_SCORE_SEED: &[u8] = b"mesh-relay-score";
pub const RELAY_CREDIT_SEED: &[u8] = b"mesh-relay-credit";

#[account]
#[derive(InitSpace)]
pub struct RelayConfig {
    pub authority: Pubkey,
    pub relay_mint: Pubkey,
    pub emissions_enabled: bool,
    pub relay_bps_of_withdraw_fee: u16,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct NodeRecord {
    pub wallet: Pubkey,
    pub mesh_short_id: [u8; 8],
    pub ack_count: u64,
    pub last_ack_height: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct RelayerScore {
    pub wallet: Pubkey,
    pub lifetime_acks: u64,
    pub last_epoch_height: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct AckRecord {
    pub height: u64,
    pub block_hash: [u8; 32],
    pub validator_index: u8,
    pub poster: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct SettleCredit {
    pub burn_txid: [u8; 32],
    pub fee: u64,
    pub attestors: [Pubkey; MAX_ATTESTORS],
    pub attestor_count: u8,
    pub claimed_bitmap: u8,
    pub bump: u8,
}

pub fn init_relay_config(
    ctx: Context<InitRelayConfig>,
    relay_bps: u16,
    emissions_enabled: bool,
) -> Result<()> {
    require!(relay_bps > 0 && relay_bps <= 10_000, BridgeError::InvalidFee);
    require!(!emissions_enabled, BridgeError::EmissionsDark);
    let rc = &mut ctx.accounts.relay_config;
    rc.authority = ctx.accounts.authority.key();
    rc.relay_mint = Pubkey::default();
    rc.emissions_enabled = false;
    rc.relay_bps_of_withdraw_fee = relay_bps;
    rc.bump = ctx.bumps.relay_config;
    Ok(())
}

pub fn set_emissions_enabled(
    ctx: Context<AuthRelay>,
    enabled: bool,
    relay_mint: Pubkey,
) -> Result<()> {
    ctx.accounts.relay_config.emissions_enabled = enabled;
    ctx.accounts.relay_config.relay_mint = relay_mint;
    Ok(())
}

pub fn register_node(ctx: Context<RegisterNode>, mesh_short_id: [u8; 8]) -> Result<()> {
    require!(mesh_short_id != [0u8; 8], BridgeError::InvalidMeshId);
    let node = &mut ctx.accounts.node;
    node.wallet = ctx.accounts.wallet.key();
    node.mesh_short_id = mesh_short_id;
    node.ack_count = 0;
    node.last_ack_height = 0;
    node.bump = ctx.bumps.node;
    let score = &mut ctx.accounts.score;
    score.wallet = ctx.accounts.wallet.key();
    score.lifetime_acks = 0;
    score.last_epoch_height = 0;
    score.bump = ctx.bumps.score;
    Ok(())
}

pub fn post_air_ack(
    ctx: Context<PostAirAck>,
    height: u64,
    block_hash: [u8; 32],
    validator_index: u8,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        (validator_index as usize) < config.attestor_count as usize,
        BridgeError::InvalidValidatorIndex
    );
    require!(
        ctx.accounts.validator.key() == config.attestors[validator_index as usize],
        BridgeError::NotAttestor
    );
    let ack = &mut ctx.accounts.ack;
    ack.height = height;
    ack.block_hash = block_hash;
    ack.validator_index = validator_index;
    ack.poster = ctx.accounts.validator.key();
    ack.bump = ctx.bumps.ack;
    ctx.accounts.node.ack_count = ctx.accounts.node.ack_count.saturating_add(1);
    ctx.accounts.node.last_ack_height = height;
    ctx.accounts.score.lifetime_acks = ctx.accounts.score.lifetime_acks.saturating_add(1);
    ctx.accounts.score.last_epoch_height = height;
    Ok(())
}

pub fn open_settle_credit(ctx: Context<OpenSettleCredit>, _burn_txid: [u8; 32]) -> Result<()> {
    let withdraw = &ctx.accounts.withdraw_record;
    require!(withdraw.fee > 0, BridgeError::ZeroAmount);
    let credit = &mut ctx.accounts.settle_credit;
    credit.burn_txid = withdraw.burn_txid;
    credit.fee = withdraw.fee;
    credit.attestors = ctx.accounts.config.attestors;
    credit.attestor_count = ctx.accounts.config.attestor_count;
    credit.claimed_bitmap = 0;
    credit.bump = ctx.bumps.settle_credit;
    Ok(())
}

pub fn claim_settle_fee(ctx: Context<ClaimSettleFee>, index: u8) -> Result<()> {
    let credit = &mut ctx.accounts.settle_credit;
    require!((index as usize) < credit.attestor_count as usize, BridgeError::NotSettleAttestor);
    require!(
        ctx.accounts.claimant.key() == credit.attestors[index as usize],
        BridgeError::NotSettleAttestor
    );
    let bit = 1u8 << index;
    require!(credit.claimed_bitmap & bit == 0, BridgeError::FeeAlreadyClaimed);
    let pot = credit
        .fee
        .checked_mul(ctx.accounts.relay_config.relay_bps_of_withdraw_fee as u64)
        .ok_or(BridgeError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(BridgeError::MathOverflow)?;
    let share = pot
        .checked_div(credit.attestor_count as u64)
        .ok_or(BridgeError::MathOverflow)?;
    require!(share > 0, BridgeError::ZeroAmount);
    let vault = ctx.accounts.sol_vault.to_account_info();
    let dest = ctx.accounts.claimant.to_account_info();
    let min_rent = Rent::get()?.minimum_balance(8 + SolVault::INIT_SPACE);
    require!(vault.lamports().saturating_sub(share) >= min_rent, BridgeError::InsufficientVault);
    **vault.try_borrow_mut_lamports()? -= share;
    **dest.try_borrow_mut_lamports()? += share;
    credit.claimed_bitmap |= bit;
    Ok(())
}

pub fn claim_settle(_ctx: Context<ClaimSettle>, _index: u8) -> Result<()> {
    err!(BridgeError::EmissionsDark)
}

#[derive(Accounts)]
pub struct InitRelayConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(has_one = authority, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,
    #[account(init, payer = authority, space = 8 + RelayConfig::INIT_SPACE, seeds = [RELAY_CONFIG_SEED], bump)]
    pub relay_config: Account<'info, RelayConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AuthRelay<'info> {
    pub authority: Signer<'info>,
    #[account(mut, has_one = authority, seeds = [RELAY_CONFIG_SEED], bump = relay_config.bump)]
    pub relay_config: Account<'info, RelayConfig>,
}

#[derive(Accounts)]
#[instruction(mesh_short_id: [u8; 8])]
pub struct RegisterNode<'info> {
    #[account(mut)]
    pub wallet: Signer<'info>,
    #[account(init, payer = wallet, space = 8 + NodeRecord::INIT_SPACE, seeds = [RELAY_NODE_SEED, mesh_short_id.as_ref()], bump)]
    pub node: Account<'info, NodeRecord>,
    #[account(init, payer = wallet, space = 8 + RelayerScore::INIT_SPACE, seeds = [RELAY_SCORE_SEED, wallet.key().as_ref()], bump)]
    pub score: Account<'info, RelayerScore>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(height: u64, block_hash: [u8; 32], validator_index: u8)]
pub struct PostAirAck<'info> {
    pub validator: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,
    #[account(init, payer = validator, space = 8 + AckRecord::INIT_SPACE, seeds = [RELAY_ACK_SEED, &height.to_le_bytes(), &[validator_index]], bump)]
    pub ack: Account<'info, AckRecord>,
    #[account(mut, seeds = [RELAY_NODE_SEED, node.mesh_short_id.as_ref()], bump = node.bump)]
    pub node: Account<'info, NodeRecord>,
    #[account(mut, seeds = [RELAY_SCORE_SEED, validator.key().as_ref()], bump = score.bump, constraint = score.wallet == validator.key())]
    pub score: Account<'info, RelayerScore>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(burn_txid: [u8; 32])]
pub struct OpenSettleCredit<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,
    #[account(seeds = [WITHDRAW_SEED, &burn_txid], bump = withdraw_record.bump)]
    pub withdraw_record: Account<'info, WithdrawRecord>,
    #[account(init, payer = payer, space = 8 + SettleCredit::INIT_SPACE, seeds = [RELAY_CREDIT_SEED, &burn_txid], bump)]
    pub settle_credit: Account<'info, SettleCredit>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(index: u8)]
pub struct ClaimSettleFee<'info> {
    #[account(mut)]
    pub claimant: Signer<'info>,
    #[account(seeds = [RELAY_CONFIG_SEED], bump = relay_config.bump)]
    pub relay_config: Account<'info, RelayConfig>,
    #[account(mut, seeds = [RELAY_CREDIT_SEED, &settle_credit.burn_txid], bump = settle_credit.bump)]
    pub settle_credit: Account<'info, SettleCredit>,
    #[account(mut, seeds = [VAULT_SEED], bump = config.vault_bump)]
    pub sol_vault: Account<'info, SolVault>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,
}

#[derive(Accounts)]
pub struct ClaimSettle<'info> {
    pub claimant: Signer<'info>,
    #[account(seeds = [RELAY_CONFIG_SEED], bump = relay_config.bump)]
    pub relay_config: Account<'info, RelayConfig>,
}
