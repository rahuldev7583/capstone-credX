use crate::{error::CredXError, CollateralVault, LoanAccount, ProtocolState, SimplePriceOracle};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{approve, mint_to, Approve, Mint, MintTo, Token, TokenAccount},
};
use pyth_sdk_solana::{
    state::{load_price_account, GenericPriceAccount},
    Price,
};

#[derive(Accounts)]
pub struct LendCreditToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = admin.key() == protocol.admin @ CredXError::UnauthorizedAdmin
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = !protocol.is_locked @ CredXError::ProtocolLocked,
        constraint = protocol.credit_mint == credit_mint.key() @ CredXError::InvalidCreditMint
    )]
    pub protocol: Account<'info, ProtocolState>,

    /// CHECK: PDA used as program authority
    #[account(seeds = [b"program_authority"], bump)]
    pub program_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        mint::decimals = 6,
        mint::authority = program_authority,
        constraint = credit_mint.key() == protocol.credit_mint @ CredXError::InvalidCreditMint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = credit_mint,
        associated_token::authority = user
    )]
    pub user_credit_ata: Account<'info, TokenAccount>,

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
    // #[account(
    //     constraint = oracle_price_account.key() == loan_account.oracle_price_account @ CredXError::InvalidOracleAccount
    // )]
    // pub oracle_price_account: AccountInfo<'info>,
    pub oracle_price_account: Account<'info, SimplePriceOracle>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> LendCreditToken<'info> {
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
        let price = self.oracle_price_account.price;

        require!(price > 0, CredXError::InvalidPrice);

        let current_time = Clock::get()?.unix_timestamp;
        let price_time = self.oracle_price_account.timestamp;
        require!(current_time - price_time < 300, CredXError::StalePrice);

        // let normalized_price = if expo < 0 {
        //     current_price
        //         .checked_div(10_i64.pow((-expo) as u32))
        //         .ok_or(CredXError::MathOverflow)?
        // } else {
        //     current_price
        //         .checked_mul(10_i64.pow(expo as u32))
        //         .ok_or(CredXError::MathOverflow)?
        // };

        // Ok(normalized_price)
        Ok(price as i64)
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

        let price = self.get_price()? as u64;
        let collateral_amount = self.loan_account.collateral_amount as u128;
        let ltv_ratio = self.protocol.ltv_ratio_bps as u128;

        let collateral_value = collateral_amount
            .checked_mul(price as u128)
            .ok_or(CredXError::MathOverflow)?;

        let borrow_value = collateral_value
            .checked_mul(ltv_ratio)
            .ok_or(CredXError::MathOverflow)?
            .checked_div(10_000)
            .ok_or(CredXError::MathOverflow)?;

        let additional_borrowable = borrow_value
            .checked_sub(self.loan_account.remaining_debt as u128)
            .ok_or(CredXError::ExceedsMaxBorrow)?;

        let borrow_amount =
            u64::try_from(additional_borrowable).map_err(|_| CredXError::MathOverflow)?;

        require!(borrow_value > 0, CredXError::ZeroBorrowAmount);

        let max_borrowable = borrow_amount;
        let current_debt = self.loan_account.remaining_debt;

        require!(additional_borrowable > 0, CredXError::MaxBorrowLimitReached);

        let accounts = MintTo {
            mint: self.credit_mint.to_account_info(),
            to: self.user_credit_ata.to_account_info(),
            authority: self.program_authority.to_account_info(),
        };

        let seeds = &[b"program_authority".as_ref(), &[bumps.program_authority]];
        let signer_seeds = &[&seeds[..]];

        let ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        mint_to(ctx, additional_borrowable as u64)?;

        let approve_accounts = Approve {
            to: self.user_credit_ata.to_account_info(),
            delegate: self.program_authority.to_account_info(),
            authority: self.user.to_account_info(), // User approves program authority
        };

        let approve_ctx = CpiContext::new(self.token_program.to_account_info(), approve_accounts);

        // Approve the full borrowed amount for future automated repayment
        approve(approve_ctx, borrow_amount)?;

        self.loan_account.remaining_debt = self
            .loan_account
            .remaining_debt
            .checked_add(additional_borrowable as u64)
            .ok_or(CredXError::MathOverflow)?;

        msg!(
            "Admin minted {} credit tokens to user: {}, Total debt: {}",
            additional_borrowable,
            self.user.key(),
            self.loan_account.remaining_debt
        );

        Ok(())
    }
}
