
use crate::error::CredXError;
use crate::ProtocolState;
use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token::{Mint, Token, TokenAccount}};

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: PDA used as program authority
    #[account(seeds = [b"program_authority"], bump)]
    pub program_authority: UncheckedAccount<'info>,
    
    #[account(
        init,
        payer = admin,
        mint::decimals = 6,
        mint::authority = program_authority, 
        seeds = [b"credit", admin.key().as_ref()],
        bump
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        init,                                  
        payer = admin,                         
        space = 8 + ProtocolState::INIT_SPACE,  
        seeds = [b"protocol", admin.key().as_ref()],
        bump
    )]
    pub protocol: Account<'info, ProtocolState>,

    #[account(
        init,
        payer = admin,
        associated_token::mint = credit_mint,
        associated_token::authority = program_authority,
    )]
    pub protocol_credit_ata: Account<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitializeProtocol<'info> {
    pub fn initialize_protocol(&mut self, bumps: &InitializeProtocolBumps) -> Result<()> {
       
        let ltv_ratio_bps = 6000; 
        require!(
            ltv_ratio_bps > 0 && ltv_ratio_bps <= 9000,
            CredXError::InvalidLtvRatio
        );
        self.protocol.set_inner(ProtocolState {
            admin: self.admin.key(),
            ltv_ratio_bps,
            credit_mint: self.credit_mint.key(),
            is_locked: false,
            bump: bumps.protocol,
        });
        msg!("Protocol initialized by admin: {}", self.admin.key());
        msg!(
            "Credit mint created with admin as authority: {}",
            self.credit_mint.key()
        );
        msg!("LTV ratio set to: {}%", ltv_ratio_bps / 100);
        Ok(())
    }
}
