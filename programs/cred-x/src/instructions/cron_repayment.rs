use crate::SimplePriceOracle;
use crate::{error::CredXError, CollateralVault, LoanAccount, ProtocolState};
use anchor_lang::prelude::*;
use anchor_spl::token::{burn, Burn, Mint, Token, TokenAccount};
use pyth_sdk_solana::state::{load_price_account, GenericPriceAccount};
use pyth_sdk_solana::Price;

#[derive(Accounts)]
pub struct CronRepayment<'info> {

    #[account(
        mut, 
        constraint = !protocol.is_locked @ CredXError::ProtocolLocked
    )]
    pub protocol: Account<'info, ProtocolState>,

    #[account(
        mut,
        seeds = [b"collateral_vault", loan_account.user.as_ref()],
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
        seeds = [b"loan", loan_account.user.as_ref(), collateral_vault.key().as_ref()],
        bump,
        constraint = loan_account.remaining_debt > 0 @ CredXError::NoOutstandingDebt
    )]
    pub loan_account: Account<'info, LoanAccount>,

    #[account(
        mut,
        mint::decimals = 6,
        constraint = credit_mint.key() == protocol.credit_mint @ CredXError::InvalidCreditMint
    )]
    pub credit_mint: Account<'info, Mint>,

    /// CHECK: PDA used as program authority
    #[account(seeds = [b"program_authority"], bump)]
    pub program_authority: UncheckedAccount<'info>,

     #[account(
        mut,
        associated_token::mint = credit_mint,
        associated_token::authority = program_authority,
    )]
    pub protocol_credit_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = credit_mint,
        associated_token::authority = loan_account.user,
        constraint = user_credit_ata.amount > 0 @ CredXError::NoTokensToBurn
    )]
    pub user_credit_ata: Account<'info, TokenAccount>,
    /// CHECK: Oracle price account is validated by comparing its key with the stored oracle_price_account in loan_account
    // #[account(
    //     constraint = oracle_price_account.key() == loan_account.oracle_price_account @ CredXError::InvalidOracleAccount
    // )]
    // pub oracle_price_account: AccountInfo<'info>,

    pub oracle_price_account: Account<'info, SimplePriceOracle>,

    pub token_program: Program<'info, Token>,
}

impl<'info> CronRepayment<'info> {
    pub fn get_price(&mut self) -> Result<i64> {
        // require!(
        //     !self.oracle_price_account.data_is_empty(),
        //     CredXError::EmptyOracleAccount
        // );
        // let price_feed = self.oracle_price_account.try_borrow_data()
        //     .map_err(|_| CredXError::FailedToBorrowOracleData)?;

        // let price_account: &GenericPriceAccount<32, Price> = load_price_account(&price_feed)
        //     .map_err(|_| CredXError::FailedToLoadPriceAccount)?;
        // let current_price = price_account.agg.price;

        // let expo = price_account.expo;

        // require!(current_price > 0, CredXError::InvalidPrice);

        // let current_time = Clock::get()?.unix_timestamp;
        // let price_time = price_account.agg.pub_slot as i64; 
        // require!(
        //     current_time - price_time < 300, 
        //     CredXError::StalePrice
        // );

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
        let current_price = self.oracle_price_account.price;
        require!(current_price > 0, CredXError::InvalidPrice);

        let current_time = Clock::get()?.unix_timestamp;
        let price_time = self.oracle_price_account.timestamp;
        require!(current_time - price_time < 300, CredXError::StalePrice);

        Ok(current_price as i64)
    }

    pub fn cron_repayment(&mut self, bumps: &CronRepaymentBumps) -> Result<()> {
        require!(!self.protocol.is_locked, CredXError::ProtocolLocked);
        require!(self.loan_account.remaining_debt > 0, CredXError::NoOutstandingDebt);
        require!(self.collateral_vault_ata.amount >= self.loan_account.collateral_amount, CredXError::InsufficientCollateral);

        let vault_balance = self.collateral_vault_ata.amount;
        let original_collateral = self.loan_account.collateral_amount;

        let yield_amount = vault_balance.checked_sub(original_collateral)
            .ok_or(CredXError::NegativeYield)?;

        if yield_amount == 0 {
            msg!("No yield available for repayment");
            return Ok(());
        }

        let normalized_price = self.get_price()?;
        let yield_value_in_usd = (yield_amount as i64)
            .checked_mul(normalized_price)
            .ok_or(CredXError::MathOverflow)?;

        require!(yield_value_in_usd > 0, CredXError::ZeroRepaymentValue);

        let repayment_value = yield_value_in_usd as u64;
        let actual_repayment = std::cmp::min(repayment_value, self.loan_account.remaining_debt);

        require!(self.user_credit_ata.amount >= actual_repayment, CredXError::InsufficientCreditTokens);

        let cpi_accounts = Burn { 
            mint: self.credit_mint.to_account_info(),
            from: self.user_credit_ata.to_account_info(), 
            authority: self.program_authority.to_account_info(),
    };


        let seeds = &[b"program_authority".as_ref(), &[bumps.program_authority]];
        let signer_seeds = &[&seeds[..]];

        let burn_ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        burn(burn_ctx, actual_repayment)?;

        self.loan_account.remaining_debt = self.loan_account.remaining_debt
            .checked_sub(actual_repayment)
            .ok_or(CredXError::MathUnderflow)?;

        self.loan_account.yield_earned = self.loan_account.yield_earned
            .checked_add(yield_amount)
            .ok_or(CredXError::MathOverflow)?;

        msg!(
            "Repaid {} credit tokens for user: {}, remaining debt: {}",
            actual_repayment,
            self.loan_account.user,
            self.loan_account.remaining_debt
        );

        Ok(())
    }
}
