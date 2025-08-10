use::anchor_lang::prelude::*;
use pyth_sdk_solana::state::load_price_account;

use crate::{CollateralVault, LoanAccount};

#[derive(Accounts)]
pub struct GetOraclePrice <'info> {
    #[account(mut, seeds = [b"loan", loan_account.user.key().as_ref(), collateral_vault.key().as_ref()], bump)]
    pub loan_account: Account<'info, LoanAccount>,
    pub collateral_vault: Account<'info, CollateralVault>,
    pub price_account: AccountInfo<'info>
}

impl <'info> GetOraclePrice <'info> {
    pub fn get_price(&mut self)-> Result<()>{
        let data = self.price_account.try_borrow_data()?;
        let price_feed = load_price_account(&data)?;
        let current_price = price_feed.agg.price;

        let expo = price_feed.expo;

        let normalized_price = if expo < 0 {
            current_price / 10_i64.pow((-expo) as u32)
        } else {
            current_price * 10_i64.pow(expo as u32)
        };

        Ok(())
    }
}