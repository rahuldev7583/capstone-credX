use crate::{error::CredXError, CollateralVault, LoanAccount, ProtocolState, SimplePriceOracle};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{
        burn, close_account, transfer, Burn, CloseAccount, Mint, Token, TokenAccount, Transfer,
    },
};
use pyth_sdk_solana::{
    state::{load_price_account, GenericPriceAccount},
    Price,
};

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
        mint::authority = program_authority,
        constraint = credit_mint.key() == protocol.credit_mint @ CredXError::InvalidCreditMint
    )]
    pub credit_mint: Account<'info, Mint>,
    /// CHECK: This is a PDA derived from seeds, used as program authority for various operations
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

    /// CHECK: Oracle price account is validated by comparing its key with the stored oracle_price_account in loan_account
    // #[account(
    //     constraint = oracle_price_account.key() == loan_account.oracle_price_account @ CredXError::InvalidOracleAccount
    // )]
    // pub oracle_price_account: AccountInfo<'info>,
    pub oracle_price_account: Account<'info, SimplePriceOracle>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> WithdrawCollateral<'info> {
    pub fn get_price(&mut self) -> Result<i64> {
        // require!(
        //     !self.oracle_price_account.data_is_empty(),
        //     CredXError::EmptyOracleAccount
        // );
        // let price_feed = self
        //     .oracle_price_account
        //     .try_borrow_data()
        //     .map_err(|_| CredXError::FailedToBorrowOracleData)?;

        // let price_account: &GenericPriceAccount<32, Price> =
        //     load_price_account(&price_feed).map_err(|_| CredXError::FailedToLoadPriceAccount)?;

        // let price_status = price_account.agg.status;
        // require!(
        //     price_status == pyth_sdk_solana::state::PriceStatus::Trading,
        //     CredXError::InvalidPriceStatus
        // );

        // let current_price = price_account.agg.price;

        // let expo = price_account.expo;

        // require!(current_price > 0, CredXError::InvalidPrice);

        // let current_time = Clock::get()?.unix_timestamp;
        // let price_time = price_account.agg.pub_slot as i64;
        // require!(current_time - price_time < 300, CredXError::StalePrice);

        // let normalized_price = if expo < 0 {
        //     current_price
        //         .checked_div(10_i64.pow((-expo) as u32))
        //         .ok_or(CredXError::MathOverflow)?
        // } else {
        //     current_price
        //         .checked_mul(10_i64.pow(expo as u32))
        //         .ok_or(CredXError::MathOverflow)?
        // };

        let current_price = self.oracle_price_account.price as u64;

        require!(current_price > 0, CredXError::InvalidPrice);

        let current_time = Clock::get()?.unix_timestamp;
        let price_time = self.oracle_price_account.timestamp;
        require!(current_time - price_time < 300, CredXError::StalePrice);

        require!(current_price > 0, CredXError::InvalidPrice);

        // Ok(normalized_price)
        Ok(current_price as i64)
    }

    pub fn withdraw_collateral(&mut self, bumps: &WithdrawCollateralBumps) -> Result<()> {
        require!(!self.protocol.is_locked, CredXError::ProtocolLocked);
        require!(
            self.user.key() == self.loan_account.user,
            CredXError::UnauthorizedUser
        );

        let vault_balance = self.collateral_vault_ata.amount;
        let remaining_debt = self.loan_account.remaining_debt;

        require!(remaining_debt > 0, CredXError::NoActiveLoan);

        msg!(
            "Checking withdrawal eligibility - Vault balance: {}, Remaining debt: {}",
            vault_balance,
            remaining_debt
        );

        let normalized_price = self.get_price()? as u128;

        let vault_balance_u128 = vault_balance as u128;
        let collateral_value_usd = vault_balance_u128
            .checked_mul(normalized_price)
            .ok_or(CredXError::MathOverflow)?;

        let collateral_value_u64 = if collateral_value_usd > u64::MAX as u128 {
            u64::MAX
        } else {
            collateral_value_usd as u64
        };

        msg!(
            "Current collateral value: {} USD, Remaining debt: {} USD, Price: {}",
            collateral_value_u64,
            remaining_debt,
            normalized_price
        );

        require!(
            collateral_value_u64 >= remaining_debt,
            CredXError::InsufficientCollateralValue
        );

        require!(
            self.user_credit_ata.amount >= remaining_debt,
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

        burn(burn_ctx, remaining_debt)?;

        msg!("Repaid full debt: {}", remaining_debt);

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

        let original_collateral = self.loan_account.collateral_amount;
        let yield_earned = vault_balance.checked_sub(original_collateral).unwrap_or(0);
        self.loan_account.collateral_amount = 0;
        self.loan_account.remaining_debt = 0;
        self.loan_account.yield_earned = self
            .loan_account
            .yield_earned
            .checked_add(yield_earned)
            .ok_or(CredXError::MathOverflow)?;

        msg!(
            "Loan fully closed for user {}. Collateral returned: {}, Yield earned this withdrawal: {}, Total yield: {}",
            self.user.key(),
            vault_balance,
            yield_earned,
            self.loan_account.yield_earned
        );

        Ok(())
    }
}
