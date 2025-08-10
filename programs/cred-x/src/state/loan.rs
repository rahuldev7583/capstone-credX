use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct LoanAccount {
    pub user: Pubkey,
    pub collateral_amount: u64,
    pub remaining_debt: u64,
    pub yield_earned: u64,
    pub bump: u8,
    pub oracle_price_account: Pubkey
}
