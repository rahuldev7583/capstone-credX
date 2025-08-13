use crate::{error::CredXError, CollateralVault, LoanAccount, ProtocolState};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{
        burn, close_account, transfer, Burn, CloseAccount, Mint, Token, TokenAccount, Transfer,
    },
};
use pyth_sdk_solana::state::load_price_account;

#[derive(Accounts)]
pub struct WithdrawCollateral<'info> {
    #[account(
        mut,
        constraint = user.key() == loan_account.user @ CredXError::UnauthorizedUser
    )]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !protocol.is_locked @ CredXError::ProtocolLocked
    )]
    pub protocol: Account<'info, ProtocolState>,

    #[account(
        mut,
        mint::authority = mint_authority,
        constraint = credit_mint.key() == protocol.credit_mint @ CredXError::InvalidCreditMint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(seeds = [b"program_authority"], bump)]
    pub program_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"collateral_vault", user.key().as_ref()],
        bump,
        constraint = collateral_vault.mint != credit_mint.key() @ CredXError::InvalidCollateralMint
    )]
    pub collateral_vault: Account<'info, CollateralVault>,

    #[account(
        mut,
        associated_token::mint = collateral_vault.mint,
        associated_token::authority = collateral_vault,
        constraint = collateral_vault_ata.amount >= loan_account.collateral_amount @ CredXError::InsufficientCollateral
    )]
    pub collateral_vault_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = collateral_vault.mint,
        associated_token::authority = user,
        constraint = user_collateral_ata.mint == collateral_vault.mint @ CredXError::MintMismatch
    )]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"loan", user.key().as_ref(), collateral_vault.key().as_ref()],
        bump,
        constraint = loan_account.user == user.key() @ CredXError::UnauthorizedUser
    )]
    pub loan_account: Account<'info, LoanAccount>,

    #[account(
        mut,
        associated_token::mint = credit_mint,
        associated_token::authority = user
    )]
    pub user_credit_ata: Account<'info, TokenAccount>,

    #[account(
        constraint = oracle_price_account.key() == loan_account.oracle_price_account @ CredXError::InvalidOracleAccount
    )]
    pub oracle_price_account: AccountInfo<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> WithdrawCollateral<'info> {
    pub fn get_price(&mut self) -> Result<i64> {
        require!(
            !self.oracle_price_account.data_is_empty(),
            CredXError::EmptyOracleAccount
        );
        let data = self
            .oracle_price_account
            .try_borrow_data()
            .map_err(|_| CredXError::FailedToBorrowOracleData)?;

        let price_feed = load_price_account(&data).map_err(|_| CredXError::InvalidPythAccount)?;

        let price_status = price_feed.agg.status;
        require!(
            price_status == pyth_sdk_solana::state::PriceStatus::Trading,
            CredXError::InvalidPriceStatus
        );

        let current_price = price_feed.agg.price;

        let expo = price_feed.expo;

        require!(current_price > 0, CredXError::InvalidPrice);

        let current_time = Clock::get()?.unix_timestamp;
        let price_time = price_feed.agg.pub_slot as i64;
        require!(current_time - price_time < 300, CredXError::StalePrice);

        let normalized_price = if expo < 0 {
            current_price
                .checked_div(10_i64.pow((-expo) as u32))
                .ok_or(CredXError::MathOverflow)?
        } else {
            current_price
                .checked_mul(10_i64.pow(expo as u32))
                .ok_or(CredXError::MathOverflow)?
        };

        Ok(normalized_price)
    }

    pub fn withdraw_collateral(&mut self, bumps: &WithdrawCollateralBumps) -> Result<()> {
        require!(!self.protocol.is_locked, CredXError::ProtocolLocked);

        require!(
            self.user.key() == self.loan_account.user,
            CredXError::UnauthorizedUser
        );
        let vault_balance = self.collateral_vault_ata.amount;
        require!(
            vault_balance >= self.loan_account.collateral_amount,
            CredXError::InsufficientCollateral
        );

        let original_collateral = self.loan_account.collateral_amount;
        let yield_amount = vault_balance
            .checked_sub(original_collateral)
            .ok_or(CredXError::NegativeYield)?;

        let mut remaining_debt = self.loan_account.remaining_debt;
        let mut final_yield = yield_amount;

        if remaining_debt > 0 && yield_amount > 0 {
            let normalized_price = self.get_price()?;

            let yield_value_usd = (yield_amount as i64)
                .checked_mul(normalized_price)
                .ok_or(CredXError::MathOverflow)? as u64;

            let repay_amount = yield_value_usd.min(remaining_debt);

            require!(
                self.user_credit_ata.amount >= repay_amount,
                CredXError::InsufficientCreditTokens
            );

            let burn_accounts = Burn {
                mint: self.credit_mint.to_account_info(),
                from: self.user_credit_ata.to_account_info(),
                authority: self.program_authority.to_account_info(),
            };

            let authority_seeds = &[b"program_authority".as_ref(), &[bumps.program_authority]];
            let signer_seeds = &[&authority_seeds[..]];

            let burn_ctx = CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                burn_accounts,
                signer_seeds,
            );

            burn(burn_ctx, repay_amount)?;

            remaining_debt = remaining_debt
                .checked_sub(repay_amount)
                .ok_or(CredXError::MathUnderflow)?;

            self.loan_account.remaining_debt = remaining_debt;

            let yield_used_for_repayment = repay_amount
                .checked_div(normalized_price as u64)
                .unwrap_or(0);

            final_yield = yield_amount.saturating_sub(yield_used_for_repayment);

            msg!(
                "Repaid {} debt using yield. Remaining debt: {}",
                repay_amount,
                remaining_debt
            );
        }

        if remaining_debt > 0 {
            let normalized_price = self.get_price()?;
            let required_collateral_value = remaining_debt
                .checked_mul(10000)
                .ok_or(CredXError::MathOverflow)?
                .checked_div(self.protocol.ltv_ratio_bps as u64)
                .ok_or(CredXError::MathOverflow)?;

            let required_collateral_amount = required_collateral_value
                .checked_div(normalized_price as u64)
                .ok_or(CredXError::MathOverflow)?;

            require!(
                vault_balance > required_collateral_amount,
                CredXError::InsufficientCollateralForDebt
            );

            let withdrawable_amount = vault_balance
                .checked_sub(required_collateral_amount)
                .ok_or(CredXError::InsufficientCollateral)?;

            require!(
                withdrawable_amount > 0,
                CredXError::NoWithdrawableCollateral
            );
        }

        let binding = self.user.key();
        let seeds = &[
            b"collateral_vault".as_ref(),
            binding.as_ref(),
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

        self.loan_account.collateral_amount = 0;
        self.loan_account.remaining_debt = 0;
        self.loan_account.yield_earned = self
            .loan_account
            .yield_earned
            .checked_add(final_yield)
            .ok_or(CredXError::MathOverflow)?;

        msg!(
            "User {} withdrew {} collateral tokens, total yield earned: {}",
            self.user.key(),
            vault_balance,
            self.loan_account.yield_earned
        );

        Ok(())
    }
}
