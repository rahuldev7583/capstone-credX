use anchor_lang::prelude::*;


#[account]
#[derive(InitSpace)]
pub struct ProtocolState {
    pub admin: Pubkey,
    pub ltv_ratio_bps: u16, // e.g. 6000 = 60.00%
    pub credit_mint: Pubkey,
    pub is_locked: bool,
    pub bump: u8,
}
