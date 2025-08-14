#![allow(deprecated)]
use anchor_lang::prelude::*;
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("Ces2ZsycAiQy79EKb9JPcCVosr3FzvrzWEpEy9XRZif5");

#[program]
pub mod cred_x {
    use super::*;

    pub fn initialize_protocol_account(ctx: Context<InitializeProtocol>) -> Result<()> {
        ctx.accounts.initialize_protocol(&ctx.bumps)
    }

    pub fn initialize_loan_account(
        ctx: Context<InitializeLoan>,
        collateral_mint: Pubkey,
    ) -> Result<()> {
        ctx.accounts.initialize_loan(collateral_mint, &ctx.bumps)
    }

    pub fn deposit_collateral_tokens(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
        ctx.accounts.deposit_collateral(amount)
    }

    pub fn lend_credit_tokens(ctx: Context<LendCreditToken>) -> Result<()> {
        ctx.accounts.lend_credit_token(&ctx.bumps)
    }

    pub fn process_repayment(ctx: Context<CronRepayment>) -> Result<()> {
        ctx.accounts.cron_repayment(&ctx.bumps)
    }

    pub fn withdraw_collateral_tokens(ctx: Context<WithdrawCollateral>) -> Result<()> {
        ctx.accounts.withdraw_collateral(&ctx.bumps)
    }
}
