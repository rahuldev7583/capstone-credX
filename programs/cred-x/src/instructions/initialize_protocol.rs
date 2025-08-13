use crate::error::CredXError;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

use crate::ProtocolState;

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(init, payer = admin, mint::decimals = 6, mint::authority = mint_authority, seeds = [b"credit", admin.key().as_ref(), mint_authority.key().as_ref() ], bump)]
    pub credit_mint: Account<'info, Mint>,

    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(init, payer = admin, space = ProtocolState::INIT_SPACE, seeds = [b"protocol", admin.key().as_ref()], bump)]
    pub protocol: Account<'info, ProtocolState>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> InitializeProtocol<'info> {
    pub fn initialize_protocol(&mut self, bumps: &InitializeProtocolBumps) -> Result<()> {
        require!(
            self.protocol.ltv_ratio_bps > 0 && self.protocol.ltv_ratio_bps <= 9000,
            CredXError::InvalidLtvRatio
        );

        self.protocol.set_inner(ProtocolState {
            admin: self.admin.key(),
            ltv_ratio_bps: 600,
            credit_mint: self.credit_mint.key(),
            is_locked: false,
            bump: bumps.protocol,
        });
        msg!("Protocol initialized by admin: {}", self.admin.key());
        Ok(())
    }
}
