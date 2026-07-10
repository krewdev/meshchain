//! MeshChain Solana vault bridge — **hybrid lock**.
//!
//! Funds deposited here are bound to a **Meshtastic mesh_short_id**.
//! When hybrid mode is on, internet parties **cannot** withdraw without:
//!   1. matching mesh_short_id (from the original deposit),
//!   2. a unique mesh burn_txid,
//!   3. ≥ K co-signers from the registered **mesh attestor** set
//!      (mesh validators who witnessed final Burn on the radio network).
//!
//! Relayer alone is not enough.

use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("CBRQcjk5DLJh1HcW3XF5TmUxZsBumhiABJa6M15r3Vkx");

pub const CONFIG_SEED: &[u8] = b"mesh-bridge-config";
pub const VAULT_SEED: &[u8] = b"mesh-bridge-vault";
pub const DEPOSIT_SEED: &[u8] = b"mesh-bridge-deposit";
pub const WITHDRAW_SEED: &[u8] = b"mesh-bridge-withdraw";
pub const MAX_ATTESTORS: usize = 8;

pub const NATIVE_SOL_FLAG: u8 = 1;
pub const SPL_FLAG: u8 = 2;

#[program]
pub mod programs_mesh_bridge {
    use super::*;

    /// Initialize vault + hybrid policy.
    /// `min_attestations`: how many mesh witnesses must co-sign unlock (0 = legacy relayer-only; use ≥2 in production).
    pub fn initialize(
        ctx: Context<Initialize>,
        fee_bps: u16,
        withdraw_fee_bps: u16,
        min_attestations: u8,
        hybrid_enabled: bool,
    ) -> Result<()> {
        require!(fee_bps <= 10_000, BridgeError::InvalidFee);
        require!(withdraw_fee_bps <= 10_000, BridgeError::InvalidFee);
        require!(
            !(hybrid_enabled && min_attestations == 0),
            BridgeError::BadHybridConfig
        );
        require!(
            (min_attestations as usize) <= MAX_ATTESTORS,
            BridgeError::BadHybridConfig
        );

        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.relayer = ctx.accounts.authority.key();
        config.fee_bps = fee_bps;
        config.withdraw_fee_bps = withdraw_fee_bps;
        config.paused = false;
        config.deposit_count = 0;
        config.withdraw_count = 0;
        config.total_deposited_sol = 0;
        config.total_withdrawn_sol = 0;
        config.bump = ctx.bumps.config;
        config.vault_bump = ctx.bumps.sol_vault;
        config.hybrid_enabled = hybrid_enabled;
        config.min_attestations = min_attestations;
        config.attestor_count = 0;
        config.attestors = [Pubkey::default(); MAX_ATTESTORS];

        ctx.accounts.sol_vault.bump = ctx.bumps.sol_vault;

        msg!(
            "MeshBridge hybrid={} min_attestations={} fee_bps={}",
            hybrid_enabled,
            min_attestations,
            fee_bps
        );
        Ok(())
    }

    pub fn set_relayer(ctx: Context<AuthConfig>, new_relayer: Pubkey) -> Result<()> {
        ctx.accounts.config.relayer = new_relayer;
        Ok(())
    }

    pub fn set_paused(ctx: Context<AuthConfig>, paused: bool) -> Result<()> {
        ctx.accounts.config.paused = paused;
        Ok(())
    }

    pub fn set_fees(ctx: Context<AuthConfig>, fee_bps: u16, withdraw_fee_bps: u16) -> Result<()> {
        require!(fee_bps <= 10_000, BridgeError::InvalidFee);
        require!(withdraw_fee_bps <= 10_000, BridgeError::InvalidFee);
        ctx.accounts.config.fee_bps = fee_bps;
        ctx.accounts.config.withdraw_fee_bps = withdraw_fee_bps;
        Ok(())
    }

    pub fn set_hybrid(
        ctx: Context<AuthConfig>,
        hybrid_enabled: bool,
        min_attestations: u8,
    ) -> Result<()> {
        require!(
            !(hybrid_enabled && min_attestations == 0),
            BridgeError::BadHybridConfig
        );
        require!(
            (min_attestations as usize) <= MAX_ATTESTORS,
            BridgeError::BadHybridConfig
        );
        ctx.accounts.config.hybrid_enabled = hybrid_enabled;
        ctx.accounts.config.min_attestations = min_attestations;
        Ok(())
    }

    /// Register Solana pubkeys of mesh validators / bridge witnesses.
    /// These must co-sign hybrid unlocks (they attest mesh Burn finality).
    pub fn set_attestors(ctx: Context<AuthConfig>, attestors: Vec<Pubkey>) -> Result<()> {
        require!(attestors.len() <= MAX_ATTESTORS, BridgeError::TooManyAttestors);
        require!(!attestors.is_empty(), BridgeError::BadHybridConfig);
        let config = &mut ctx.accounts.config;
        config.attestors = [Pubkey::default(); MAX_ATTESTORS];
        for (i, a) in attestors.iter().enumerate() {
            config.attestors[i] = *a;
        }
        config.attestor_count = attestors.len() as u8;
        require!(
            config.min_attestations <= config.attestor_count,
            BridgeError::BadHybridConfig
        );
        Ok(())
    }

    /// Lock SOL to a **Meshtastic short id**. Internet cannot free this without mesh proof.
    pub fn deposit_sol(
        ctx: Context<DepositSol>,
        amount: u64,
        mesh_short_id: [u8; 8],
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(!config.paused, BridgeError::Paused);
        require!(amount > 0, BridgeError::ZeroAmount);
        // Reject all-zero short id (must name a real mesh wallet)
        require!(mesh_short_id != [0u8; 8], BridgeError::InvalidMeshId);

        let fee = mul_bps(amount, config.fee_bps)?;
        let amount_net = amount.checked_sub(fee).ok_or(BridgeError::MathOverflow)?;
        require!(amount_net > 0, BridgeError::ZeroAmount);

        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.depositor.to_account_info(),
                    to: ctx.accounts.sol_vault.to_account_info(),
                },
            ),
            amount,
        )?;

        let seq = config.deposit_count;
        config.deposit_count = config.deposit_count.checked_add(1).ok_or(BridgeError::MathOverflow)?;
        config.total_deposited_sol = config
            .total_deposited_sol
            .checked_add(amount)
            .ok_or(BridgeError::MathOverflow)?;

        let deposit = &mut ctx.accounts.deposit_record;
        deposit.seq = seq;
        deposit.depositor = ctx.accounts.depositor.key();
        deposit.mesh_short_id = mesh_short_id;
        deposit.amount_gross = amount;
        deposit.amount_net = amount_net;
        deposit.amount_unlocked = 0;
        deposit.fee = fee;
        deposit.asset_flag = NATIVE_SOL_FLAG;
        deposit.mint = Pubkey::default();
        deposit.bump = ctx.bumps.deposit_record;

        emit!(DepositEvent {
            seq,
            depositor: deposit.depositor,
            mesh_short_id,
            amount_gross: amount,
            amount_net,
            fee,
            asset_flag: NATIVE_SOL_FLAG,
            mint: Pubkey::default(),
            hybrid: config.hybrid_enabled,
        });

        msg!(
            "hybrid-lock deposit seq={} net={} mesh_id={}",
            seq,
            amount_net,
            hex8(&mesh_short_id)
        );
        Ok(())
    }

    pub fn deposit_spl(
        ctx: Context<DepositSpl>,
        amount: u64,
        mesh_short_id: [u8; 8],
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(!config.paused, BridgeError::Paused);
        require!(amount > 0, BridgeError::ZeroAmount);
        require!(mesh_short_id != [0u8; 8], BridgeError::InvalidMeshId);

        let fee = mul_bps(amount, config.fee_bps)?;
        let amount_net = amount.checked_sub(fee).ok_or(BridgeError::MathOverflow)?;
        require!(amount_net > 0, BridgeError::ZeroAmount);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.depositor_token.to_account_info(),
                    to: ctx.accounts.vault_token.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            amount,
        )?;

        let seq = config.deposit_count;
        config.deposit_count = config.deposit_count.checked_add(1).ok_or(BridgeError::MathOverflow)?;

        let deposit = &mut ctx.accounts.deposit_record;
        deposit.seq = seq;
        deposit.depositor = ctx.accounts.depositor.key();
        deposit.mesh_short_id = mesh_short_id;
        deposit.amount_gross = amount;
        deposit.amount_net = amount_net;
        deposit.amount_unlocked = 0;
        deposit.fee = fee;
        deposit.asset_flag = SPL_FLAG;
        deposit.mint = ctx.accounts.mint.key();
        deposit.bump = ctx.bumps.deposit_record;

        emit!(DepositEvent {
            seq,
            depositor: deposit.depositor,
            mesh_short_id,
            amount_gross: amount,
            amount_net,
            fee,
            asset_flag: SPL_FLAG,
            mint: deposit.mint,
            hybrid: config.hybrid_enabled,
        });
        Ok(())
    }

    /// **Hybrid unlock** — requires mesh identity + mesh attestor co-signers.
    ///
    /// `remaining_accounts`: distinct Signer accounts that must be registered attestors.
    /// Count must be ≥ config.min_attestations when hybrid_enabled.
    ///
    /// Relayer may pay fees / submit tx, but **cannot** unlock alone under hybrid mode.
    pub fn withdraw_hybrid_sol<'info>(
        ctx: Context<'_, '_, 'info, 'info, WithdrawHybridSol<'info>>,
        burn_txid: [u8; 32],
        amount: u64,
        mesh_height: u64,
        mesh_short_id: [u8; 8],
    ) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(!config.paused, BridgeError::Paused);
        require!(amount > 0, BridgeError::ZeroAmount);
        require!(burn_txid != [0u8; 32], BridgeError::InvalidBurnTxid);

        let deposit = &mut ctx.accounts.deposit_record;
        // ── Mesh identifier binding (internet alone is not enough) ──
        require!(
            deposit.mesh_short_id == mesh_short_id,
            BridgeError::MeshIdMismatch
        );

        let remaining = deposit
            .amount_net
            .checked_sub(deposit.amount_unlocked)
            .ok_or(BridgeError::MathOverflow)?;
        require!(amount <= remaining, BridgeError::ExceedsClaim);

        // ── Fail-secure hybrid: mesh attestors required (internet alone CANNOT unlock) ──
        // Even if hybrid_enabled is false, require at least 1 attestor when any are registered.
        // Production must set hybrid_enabled=true and min_attestations>=2.
        if config.hybrid_enabled {
            require!(config.min_attestations >= 1, BridgeError::BadHybridConfig);
            require!(config.attestor_count >= config.min_attestations, BridgeError::BadHybridConfig);
            let n = count_attestor_signers(config, ctx.remaining_accounts)?;
            require!(
                n >= config.min_attestations as usize,
                BridgeError::InsufficientMeshAttestations
            );
        } else {
            // Legacy dev path only — still requires designated relayer; DO NOT use for real funds.
            require!(
                ctx.accounts.relayer.key() == config.relayer,
                BridgeError::UnauthorizedRelayer
            );
            msg!("WARNING: hybrid disabled — not safe for real value");
        }

        let fee = mul_bps(amount, config.withdraw_fee_bps)?;
        let amount_out = amount.checked_sub(fee).ok_or(BridgeError::MathOverflow)?;
        require!(amount_out > 0, BridgeError::ZeroAmount);

        let vault_info = ctx.accounts.sol_vault.to_account_info();
        let dest_info = ctx.accounts.destination.to_account_info();
        let min_rent = Rent::get()?.minimum_balance(8 + SolVault::INIT_SPACE);
        require!(
            vault_info.lamports().saturating_sub(amount_out) >= min_rent,
            BridgeError::InsufficientVault
        );

        **vault_info.try_borrow_mut_lamports()? -= amount_out;
        **dest_info.try_borrow_mut_lamports()? += amount_out;

        deposit.amount_unlocked = deposit
            .amount_unlocked
            .checked_add(amount)
            .ok_or(BridgeError::MathOverflow)?;

        let rec = &mut ctx.accounts.withdraw_record;
        rec.burn_txid = burn_txid;
        rec.destination = ctx.accounts.destination.key();
        rec.amount_gross = amount;
        rec.amount_out = amount_out;
        rec.fee = fee;
        rec.mesh_height = mesh_height;
        rec.mesh_short_id = mesh_short_id;
        rec.deposit_seq = deposit.seq;
        rec.asset_flag = NATIVE_SOL_FLAG;
        rec.mint = Pubkey::default();
        rec.bump = ctx.bumps.withdraw_record;
        rec.attestation_count = if config.hybrid_enabled {
            count_attestor_signers(config, ctx.remaining_accounts)? as u8
        } else {
            0
        };

        let config = &mut ctx.accounts.config;
        config.withdraw_count = config
            .withdraw_count
            .checked_add(1)
            .ok_or(BridgeError::MathOverflow)?;
        config.total_withdrawn_sol = config
            .total_withdrawn_sol
            .checked_add(amount_out)
            .ok_or(BridgeError::MathOverflow)?;

        emit!(WithdrawEvent {
            burn_txid,
            destination: rec.destination,
            amount_gross: amount,
            amount_out,
            fee,
            mesh_height,
            mesh_short_id,
            deposit_seq: rec.deposit_seq,
            asset_flag: NATIVE_SOL_FLAG,
            mint: Pubkey::default(),
            hybrid: config.hybrid_enabled,
            attestation_count: rec.attestation_count,
        });

        msg!(
            "hybrid-unlock out={} mesh_id={} burn=… height={}",
            amount_out,
            hex8(&mesh_short_id),
            mesh_height
        );
        Ok(())
    }

    /// SPL hybrid unlock (same mesh binding + attestors).
    pub fn withdraw_hybrid_spl<'info>(
        ctx: Context<'_, '_, 'info, 'info, WithdrawHybridSpl<'info>>,
        burn_txid: [u8; 32],
        amount: u64,
        mesh_height: u64,
        mesh_short_id: [u8; 8],
    ) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(!config.paused, BridgeError::Paused);
        require!(amount > 0, BridgeError::ZeroAmount);
        require!(burn_txid != [0u8; 32], BridgeError::InvalidBurnTxid);

        let deposit = &mut ctx.accounts.deposit_record;
        require!(
            deposit.mesh_short_id == mesh_short_id,
            BridgeError::MeshIdMismatch
        );
        require!(deposit.mint == ctx.accounts.mint.key(), BridgeError::MintMismatch);

        let remaining = deposit
            .amount_net
            .checked_sub(deposit.amount_unlocked)
            .ok_or(BridgeError::MathOverflow)?;
        require!(amount <= remaining, BridgeError::ExceedsClaim);

        if config.hybrid_enabled {
            require!(config.min_attestations >= 1, BridgeError::BadHybridConfig);
            require!(config.attestor_count >= config.min_attestations, BridgeError::BadHybridConfig);
            let n = count_attestor_signers(config, ctx.remaining_accounts)?;
            require!(
                n >= config.min_attestations as usize,
                BridgeError::InsufficientMeshAttestations
            );
        } else {
            require!(
                ctx.accounts.relayer.key() == config.relayer,
                BridgeError::UnauthorizedRelayer
            );
            msg!("WARNING: hybrid disabled — not safe for real value");
        }

        let fee = mul_bps(amount, config.withdraw_fee_bps)?;
        let amount_out = amount.checked_sub(fee).ok_or(BridgeError::MathOverflow)?;
        require!(amount_out > 0, BridgeError::ZeroAmount);

        let seeds: &[&[u8]] = &[CONFIG_SEED, &[config.bump]];
        let signer = &[seeds];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_token.to_account_info(),
                    to: ctx.accounts.destination_token.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer,
            ),
            amount_out,
        )?;

        deposit.amount_unlocked = deposit
            .amount_unlocked
            .checked_add(amount)
            .ok_or(BridgeError::MathOverflow)?;

        let hybrid = config.hybrid_enabled;
        let att_count = if hybrid {
            count_attestor_signers(config, ctx.remaining_accounts)? as u8
        } else {
            0
        };
        let deposit_seq = deposit.seq;
        let mint_key = ctx.accounts.mint.key();

        let rec = &mut ctx.accounts.withdraw_record;
        rec.burn_txid = burn_txid;
        rec.destination = ctx.accounts.destination.key();
        rec.amount_gross = amount;
        rec.amount_out = amount_out;
        rec.fee = fee;
        rec.mesh_height = mesh_height;
        rec.mesh_short_id = mesh_short_id;
        rec.deposit_seq = deposit_seq;
        rec.asset_flag = SPL_FLAG;
        rec.mint = mint_key;
        rec.bump = ctx.bumps.withdraw_record;
        rec.attestation_count = att_count;

        let config = &mut ctx.accounts.config;
        config.withdraw_count = config
            .withdraw_count
            .checked_add(1)
            .ok_or(BridgeError::MathOverflow)?;

        emit!(WithdrawEvent {
            burn_txid,
            destination: rec.destination,
            amount_gross: amount,
            amount_out,
            fee,
            mesh_height,
            mesh_short_id,
            deposit_seq,
            asset_flag: SPL_FLAG,
            mint: mint_key,
            hybrid,
            attestation_count: att_count,
        });
        Ok(())
    }
}

fn mul_bps(amount: u64, bps: u16) -> Result<u64> {
    let n = amount
        .checked_mul(bps as u64)
        .ok_or(error!(BridgeError::MathOverflow))?;
    n.checked_div(10_000)
        .ok_or_else(|| error!(BridgeError::MathOverflow))
}

fn hex8(id: &[u8; 8]) -> String {
    id.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Count unique remaining accounts that are Signers and listed as attestors.
fn count_attestor_signers(config: &BridgeConfig, remaining: &[AccountInfo]) -> Result<usize> {
    let mut seen = Vec::new();
    let mut count = 0usize;
    for acc in remaining {
        if !acc.is_signer {
            continue;
        }
        let k = acc.key();
        if seen.contains(&k) {
            continue;
        }
        let mut is_attestor = false;
        for i in 0..(config.attestor_count as usize) {
            if config.attestors[i] == k {
                is_attestor = true;
                break;
            }
        }
        if is_attestor {
            seen.push(k);
            count += 1;
        }
    }
    Ok(count)
}

// ─── Accounts ───────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + BridgeConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = authority,
        space = 8 + SolVault::INIT_SPACE,
        seeds = [VAULT_SEED],
        bump
    )]
    pub sol_vault: Account<'info, SolVault>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AuthConfig<'info> {
    pub authority: Signer<'info>,
    #[account(mut, has_one = authority, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,
}

#[derive(Accounts)]
#[instruction(amount: u64, mesh_short_id: [u8; 8])]
pub struct DepositSol<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,

    #[account(mut, seeds = [VAULT_SEED], bump = config.vault_bump)]
    pub sol_vault: Account<'info, SolVault>,

    #[account(
        init,
        payer = depositor,
        space = 8 + DepositRecord::INIT_SPACE,
        seeds = [DEPOSIT_SEED, &config.deposit_count.to_le_bytes()],
        bump
    )]
    pub deposit_record: Account<'info, DepositRecord>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64, mesh_short_id: [u8; 8])]
pub struct DepositSpl<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = depositor_token.owner == depositor.key(),
        constraint = depositor_token.mint == mint.key()
    )]
    pub depositor_token: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = vault_token.mint == mint.key(),
        constraint = vault_token.owner == config.key()
    )]
    pub vault_token: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = depositor,
        space = 8 + DepositRecord::INIT_SPACE,
        seeds = [DEPOSIT_SEED, &config.deposit_count.to_le_bytes()],
        bump
    )]
    pub deposit_record: Account<'info, DepositRecord>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(burn_txid: [u8; 32], amount: u64, mesh_height: u64, mesh_short_id: [u8; 8])]
pub struct WithdrawHybridSol<'info> {
    /// Pays rent for withdraw record; may be relayer. Not sufficient alone under hybrid.
    #[account(mut)]
    pub relayer: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,

    #[account(mut, seeds = [VAULT_SEED], bump = config.vault_bump)]
    pub sol_vault: Account<'info, SolVault>,

    /// CHECK: destination receives SOL
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,

    /// Original deposit that bound funds to mesh_short_id
    #[account(
        mut,
        seeds = [DEPOSIT_SEED, &deposit_record.seq.to_le_bytes()],
        bump = deposit_record.bump
    )]
    pub deposit_record: Account<'info, DepositRecord>,

    #[account(
        init,
        payer = relayer,
        space = 8 + WithdrawRecord::INIT_SPACE,
        seeds = [WITHDRAW_SEED, &burn_txid],
        bump
    )]
    pub withdraw_record: Account<'info, WithdrawRecord>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(burn_txid: [u8; 32], amount: u64, mesh_height: u64, mesh_short_id: [u8; 8])]
pub struct WithdrawHybridSpl<'info> {
    #[account(mut)]
    pub relayer: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, BridgeConfig>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_token.mint == mint.key(),
        constraint = vault_token.owner == config.key()
    )]
    pub vault_token: Account<'info, TokenAccount>,

    /// CHECK: destination wallet
    pub destination: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = destination_token.owner == destination.key(),
        constraint = destination_token.mint == mint.key()
    )]
    pub destination_token: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [DEPOSIT_SEED, &deposit_record.seq.to_le_bytes()],
        bump = deposit_record.bump
    )]
    pub deposit_record: Account<'info, DepositRecord>,

    #[account(
        init,
        payer = relayer,
        space = 8 + WithdrawRecord::INIT_SPACE,
        seeds = [WITHDRAW_SEED, &burn_txid],
        bump
    )]
    pub withdraw_record: Account<'info, WithdrawRecord>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// ─── State ──────────────────────────────────────────────────────────────────

#[account]
#[derive(InitSpace)]
pub struct BridgeConfig {
    pub authority: Pubkey,
    pub relayer: Pubkey,
    pub fee_bps: u16,
    pub withdraw_fee_bps: u16,
    pub paused: bool,
    pub deposit_count: u64,
    pub withdraw_count: u64,
    pub total_deposited_sol: u64,
    pub total_withdrawn_sol: u64,
    pub bump: u8,
    pub vault_bump: u8,
    /// When true, unlock needs mesh id match + K attestor co-signers.
    pub hybrid_enabled: bool,
    pub min_attestations: u8,
    pub attestor_count: u8,
    pub attestors: [Pubkey; MAX_ATTESTORS],
}

#[account]
#[derive(InitSpace)]
pub struct SolVault {
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct DepositRecord {
    pub seq: u64,
    pub depositor: Pubkey,
    /// Meshtastic / MeshChain short id funds are locked to.
    pub mesh_short_id: [u8; 8],
    pub amount_gross: u64,
    pub amount_net: u64,
    /// How much of amount_net has already been unlocked.
    pub amount_unlocked: u64,
    pub fee: u64,
    pub asset_flag: u8,
    pub mint: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct WithdrawRecord {
    pub burn_txid: [u8; 32],
    pub destination: Pubkey,
    pub amount_gross: u64,
    pub amount_out: u64,
    pub fee: u64,
    pub mesh_height: u64,
    pub mesh_short_id: [u8; 8],
    pub deposit_seq: u64,
    pub asset_flag: u8,
    pub mint: Pubkey,
    pub attestation_count: u8,
    pub bump: u8,
}

// ─── Events ─────────────────────────────────────────────────────────────────

#[event]
pub struct DepositEvent {
    pub seq: u64,
    pub depositor: Pubkey,
    pub mesh_short_id: [u8; 8],
    pub amount_gross: u64,
    pub amount_net: u64,
    pub fee: u64,
    pub asset_flag: u8,
    pub mint: Pubkey,
    pub hybrid: bool,
}

#[event]
pub struct WithdrawEvent {
    pub burn_txid: [u8; 32],
    pub destination: Pubkey,
    pub amount_gross: u64,
    pub amount_out: u64,
    pub fee: u64,
    pub mesh_height: u64,
    pub mesh_short_id: [u8; 8],
    pub deposit_seq: u64,
    pub asset_flag: u8,
    pub mint: Pubkey,
    pub hybrid: bool,
    pub attestation_count: u8,
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[error_code]
pub enum BridgeError {
    #[msg("Invalid fee bps")]
    InvalidFee,
    #[msg("Bridge is paused")]
    Paused,
    #[msg("Amount must be > 0")]
    ZeroAmount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Unauthorized relayer")]
    UnauthorizedRelayer,
    #[msg("Insufficient vault balance")]
    InsufficientVault,
    #[msg("Hybrid mode needs min_attestations >= 1")]
    BadHybridConfig,
    #[msg("Too many attestors")]
    TooManyAttestors,
    #[msg("Mesh short id is required and must not be zero")]
    InvalidMeshId,
    #[msg("Mesh short id does not match the locked deposit")]
    MeshIdMismatch,
    #[msg("Not enough mesh attestor co-signers — internet alone cannot unlock")]
    InsufficientMeshAttestations,
    #[msg("Invalid mesh burn tx id")]
    InvalidBurnTxid,
    #[msg("Amount exceeds remaining locked claim")]
    ExceedsClaim,
    #[msg("Mint mismatch for this deposit")]
    MintMismatch,
}
