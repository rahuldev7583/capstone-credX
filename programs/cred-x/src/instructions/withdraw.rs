use crate::{CollateralVault, LoanAccount, ProtocolState};
use anchor_lang::prelude::*;
use anchor_spl::token::{close_account, transfer, CloseAccount, Token, TokenAccount, Transfer};
use pyth_sdk_solana::state::load_price_account;

#[derive(Accounts)]
pub struct WithdrawCollateral<'info> {
    #[account(mut)]
    pub protocol: Account<'info, ProtocolState>,

    #[account(mut, seeds = [b"collateral_vault", loan_account.user.as_ref()], bump)]
    pub collateral_vault: Account<'info, CollateralVault>,

    #[account(
        mut,
        associated_token::mint = collateral_vault.mint,
        associated_token::authority = collateral_vault
    )]
    pub collateral_vault_ata: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"loan", loan_account.user.as_ref(), collateral_vault.key().as_ref()], bump)]
    pub loan_account: Account<'info, LoanAccount>,

    #[account(mut)]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    #[account(mut, address = loan_account.user)]
    pub user: SystemAccount<'info>,

    pub price_account: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

impl<'info> WithdrawCollateral<'info> {
    pub fn withdraw_collateral(&mut self, bumps: &WithdrawCollateralBumps) -> Result<()> {
        let vault_balance = self.collateral_vault_ata.amount;
        let yield_amount = vault_balance
            .checked_sub(self.loan_account.collateral_amount)
            .unwrap_or(0);

        if self.loan_account.remaining_debt > 0 && yield_amount > 0 {
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
            let repay_amount = repayment_value.min(self.loan_account.remaining_debt);
            self.loan_account.remaining_debt -= repay_amount;
        }
        let seeds = &[
            b"collateral_vault".as_ref(),
            self.loan_account.user.as_ref(),
            &[bumps.collateral_vault],
        ];
        let signer_seeds = &[&seeds[..]];

        let transfer_ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            Transfer {
                from: self.collateral_vault_ata.to_account_info(),
                to: self.user_collateral_ata.to_account_info(),
                authority: self.collateral_vault.to_account_info(),
            },
            signer_seeds,
        );
        transfer(transfer_ctx, vault_balance)?;

        let close_ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            CloseAccount {
                account: self.collateral_vault_ata.to_account_info(),
                destination: self.user.to_account_info(),
                authority: self.collateral_vault.to_account_info(),
            },
            signer_seeds,
        );
        close_account(close_ctx)?;

        Ok(())
    }
}
