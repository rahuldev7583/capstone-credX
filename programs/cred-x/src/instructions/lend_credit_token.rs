use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use pyth_sdk_solana::{state::{load_price_account, GenericPriceAccount}, Price};

use crate::{error::CredXError, CollateralVault, LoanAccount, ProtocolState};

#[derive(Accounts)]
pub struct LendCreditToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = !protocol.is_locked @ CredXError::ProtocolLocked, constraint = protocol.credit_mint != credit_mint.key() @ CredXError::InvalidCreditMint
    )]
    pub protocol: Account<'info, ProtocolState>,

    #[account(
        mut,
        mint::decimals = 6,
        mint::authority = mint_authority,
        constraint = credit_mint.key() == protocol.credit_mint @ CredXError::InvalidCreditMint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        mut, 
        associated_token::mint = credit_mint, 
        associated_token::authority = user
    )]
    pub user_credit_ata: Account<'info, TokenAccount>,

    /// CHECK: This is a PDA derived from seeds, used as mint authority for credit tokens
    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(
        mut, 
        seeds = [b"collateral_vault", user.key().as_ref()], 
        bump,
        constraint = collateral_vault.mint != credit_mint.key() @ CredXError::InvalidCollateralMint
    )]
    pub collateral_vault: Account<'info, CollateralVault>,

    #[account(
        mut, 
        seeds = [b"loan", user.key().as_ref(), collateral_vault.key().as_ref()], 
        bump,
        constraint = loan_account.user == user.key() @ CredXError::UnauthorizedUser,
        constraint = loan_account.collateral_amount > 0 @ CredXError::NoCollateralDeposited
    )]
    pub loan_account: Account<'info, LoanAccount>,

    /// CHECK: Oracle price account is validated by comparing its key with the stored oracle_price_account in loan_account
    #[account(
        constraint = oracle_price_account.key() == loan_account.oracle_price_account @ CredXError::InvalidOracleAccount
    )]
    pub oracle_price_account: AccountInfo<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> LendCreditToken<'info> {
    pub fn get_price(&mut self) -> Result<i64> {
        require!(
            !self.oracle_price_account.data_is_empty(),
            CredXError::EmptyOracleAccount
        );
        let price_feed = self.oracle_price_account.try_borrow_data()
            .map_err(|_| CredXError::FailedToBorrowOracleData)?;

        let price_account: &GenericPriceAccount<32, Price> = load_price_account(&price_feed)
            .map_err(|_| CredXError::FailedToLoadPriceAccount)?;

        let price_status = price_account.agg.status;
        require!(
            price_status == pyth_sdk_solana::state::PriceStatus::Trading,
            CredXError::InvalidPriceStatus
        );

        let current_price = price_account.agg.price;

        let expo = price_account.expo;

        require!(current_price > 0, CredXError::InvalidPrice);

        let current_time = Clock::get()?.unix_timestamp;
        let price_time = price_account.agg.pub_slot as i64; 
        require!(
            current_time - price_time < 300, 
            CredXError::StalePrice
        );

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

    pub fn lend_credit_token(&mut self, bumps: &LendCreditTokenBumps) -> Result<()> {
        require!(!self.protocol.is_locked, CredXError::ProtocolLocked);

        require!(
            self.loan_account.collateral_amount > 0,
            CredXError::NoCollateralDeposited
        );
        
        require!(
            self.protocol.ltv_ratio_bps > 0 && self.protocol.ltv_ratio_bps <= 9000,
            CredXError::InvalidLtvRatio
        );


        let price = self.get_price()?;

         let collateral_value = self.loan_account.collateral_amount
            .checked_mul(price as u64)
            .ok_or(CredXError::MathOverflow)?;
        
        let borrow_value = collateral_value
            .checked_mul(self.protocol.ltv_ratio_bps as u64)
            .ok_or(CredXError::MathOverflow)?
            .checked_div(10000)
            .ok_or(CredXError::MathOverflow)?;
        
        require!(borrow_value > 0, CredXError::ZeroBorrowAmount);
        
        let borrow_amount = u64::try_from(borrow_value)
            .map_err(|_| CredXError::InvalidBorrowAmount)?;

        let max_borrowable = borrow_amount;
        let current_debt = self.loan_account.remaining_debt;
        let additional_borrowable = max_borrowable
            .checked_sub(current_debt)
            .ok_or(CredXError::ExceedsMaxBorrow)?;
        
        require!(
            additional_borrowable > 0,
            CredXError::MaxBorrowLimitReached
        );

        let seeds = &[b"mint_authority".as_ref(), &[bumps.mint_authority]];

        let signer_seeds = &[&seeds[..]];

        let accounts = MintTo {
            mint: self.credit_mint.to_account_info(),
            to: self.user_credit_ata.to_account_info(),
            authority: self.mint_authority.to_account_info(),
        };
        let ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        mint_to(ctx, borrow_value as u64)?;

        self.loan_account.remaining_debt = self.loan_account.remaining_debt
            .checked_add(additional_borrowable)
            .ok_or(CredXError::MathOverflow)?;

        msg!(
            "Minted {} credit tokens to user: {}, Total debt: {}",
            additional_borrowable,
            self.user.key(),
            self.loan_account.remaining_debt
        );

        Ok(())
    }
}
