use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct CollateralVault{
    pub mint: Pubkey,
    pub bump: u8
}