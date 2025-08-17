#![allow(deprecated)]
use anchor_lang::prelude::*;
pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use error::*;
pub use instructions::*;
pub use state::*;

declare_id!("Ces2ZsycAiQy79EKb9JPcCVosr3FzvrzWEpEy9XRZif5");

#[program]
pub mod cred_x {
    use super::*;

    pub fn initialize_protocol(ctx: Context<InitializeProtocol>) -> Result<()> {
        ctx.accounts.initialize_protocol(&ctx.bumps)
    }

    pub fn initialize_loan(ctx: Context<InitializeLoan>, collateral_mint: Pubkey) -> Result<()> {
        ctx.accounts.initialize_loan(collateral_mint, &ctx.bumps)
    }

    pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
        ctx.accounts.deposit_collateral(amount)
    }

    pub fn lend_credit_token(ctx: Context<LendCreditToken>) -> Result<()> {
        ctx.accounts.lend_credit_token(&ctx.bumps)
    }

    pub fn cron_repayment(ctx: Context<CronRepayment>) -> Result<()> {
        ctx.accounts.cron_repayment(&ctx.bumps)
    }

    pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>) -> Result<()> {
        ctx.accounts.withdraw_collateral(&ctx.bumps)
    }
    pub fn create_simple_oracle(ctx: Context<CreateSimpleOracle>, price: u64) -> Result<()> {
        ctx.accounts.price_account.price = price;
        ctx.accounts.price_account.timestamp = Clock::get()?.unix_timestamp;
        Ok(())
    }

    pub fn update_simple_oracle(ctx: Context<UpdateSimpleOracle>, price: u64) -> Result<()> {
        ctx.accounts.price_account.price = price;
        ctx.accounts.price_account.timestamp = Clock::get()?.unix_timestamp;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateSimpleOracle<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + 8 + 8, 
    )]
    pub price_account: Account<'info, SimplePriceOracle>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateSimpleOracle<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub price_account: Account<'info, SimplePriceOracle>,
}

#[account]
pub struct SimplePriceOracle {
    pub price: u64, 
    pub timestamp: i64
}
