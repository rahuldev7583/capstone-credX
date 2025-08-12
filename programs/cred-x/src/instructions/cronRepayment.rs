use crate::{CollateralVault, LoanAccount, ProtocolState};
use anchor_lang::prelude::*;
use anchor_spl::token::{burn, Burn, Mint, Token, TokenAccount};
use pyth_sdk_solana::state::load_price_account;

#[derive(Accounts)]
pub struct CronRepayment<'info> {
    #[account(mut)]
    pub protocol: Account<'info, ProtocolState>,

    #[account(mut, seeds = [b"collateral_vault", loan_account.user.as_ref()], bump)]
    pub collateral_vault: Account<'info, CollateralVault>,

    #[account(mut, associated_token::mint = collateral_vault.mint, associated_token::authority = collateral_vault)]
    pub collateral_vault_ata: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"loan", loan_account.user.as_ref(), collateral_vault.key().as_ref()], bump)]
    pub loan_account: Account<'info, LoanAccount>,

    #[account(mut, mint::decimals = 6, mint::authority = mint_authority)]
    pub credit_mint: Account<'info, Mint>,

    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub user_credit_ata: Account<'info, TokenAccount>,

    pub price_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

impl<'info> CronRepayment<'info> {
    pub fn cron_repayment(&mut self, bumps: &CronRepaymentBumps) -> Result<()> {
        let vault_balance = self.collateral_vault_ata.amount;
        let yield_amount = vault_balance
            .checked_sub(self.loan_account.collateral_amount)
            .unwrap_or(0);

        if yield_amount == 0 {
            return Ok(());
        }

        let data = self.price_account.try_borrow_data()?;
        let price_feed = load_price_account(&data)?;
        let price = price_feed.agg.price;
        let expo = price_feed.expo;

        let normalized_price = if expo < 0 {
            price / 10_i64.pow((-expo) as u32)
        } else {
            price * 10_i64.pow(expo as u32)
        };

        let repayment_value = (yield_amount as i64 * normalized_price) as u64;

        let cpi_accounts = Burn {
            mint: self.credit_mint.to_account_info(),
            from: self.user_credit_ata.to_account_info(),
            authority: self.mint_authority.to_account_info(),
        };

        let seeds = &[b"mint_authority".as_ref(), &[bumps.mint_authority]];
        let signer_seeds = &[&seeds[..]];

        let burn_ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        burn(burn_ctx, repayment_value)?;

        self.loan_account.remaining_debt = self
            .loan_account
            .remaining_debt
            .saturating_sub(repayment_value);
        self.loan_account.yield_earned += yield_amount;

        Ok(())
    }
}
