use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use pyth_sdk_solana::state::load_price_account;

use crate::{CollateralVault, LoanAccount, ProtocolState};

#[derive(Accounts)]
pub struct LendCreditToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub protocol: Account<'info, ProtocolState>,

    #[account(mut, mint::decimals = 6, mint::authority = mint_authority)]
    pub credit_mint: Account<'info, Mint>,

    #[account(mut, associated_token::mint = credit_mint, associated_token::authority = user)]
    pub user_credit_ata: Account<'info, TokenAccount>,

    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(mut, seeds = [b"collateral_vault", user.key().as_ref()], bump)]
    pub collateral_vault: Account<'info, CollateralVault>,

    #[account(mut, seeds = [b"loan", user.key().as_ref(), collateral_vault.key().as_ref()], bump)]
    pub loan_account: Account<'info, LoanAccount>,

    pub oracle_price_account: AccountInfo<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> LendCreditToken<'info> {
    pub fn get_price(&mut self) -> Result<i64> {
        let data = self.oracle_price_account.try_borrow_data()?;
        let price_feed = load_price_account(&data)?;
        let current_price = price_feed.agg.price;

        let expo = price_feed.expo;

        let normalized_price = if expo < 0 {
            current_price / 10_i64.pow((-expo) as u32)
        } else {
            current_price * 10_i64.pow(expo as u32)
        };

        Ok(normalized_price)
    }

    pub fn lend_credit_token(&mut self, bumps: &LendCreditTokenBumps) -> Result<()> {
        let price = self.get_price()?;

        let collateral_amount = self.loan_account.collateral_amount as i64;
        let collateral_value = collateral_amount * price;

        let borrow_value = (collateral_value * self.protocol.ltv_ratio_bps as i64) / 10000;

        let seeds = &[b"mint_authority".as_ref(), &[bumps.mint_authority]];

        let signer_seeds = &[&seeds[..]];

        let accounts = MintTo {
            mint: self.credit_mint.to_account_info(),
            to: self.user_credit_ata.to_account_info(),
            authority: self.protocol.to_account_info(),
        };
        let ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        mint_to(ctx, borrow_value as u64);

        self.loan_account.remaining_debt += borrow_value as u64;

        Ok(())
    }
}
