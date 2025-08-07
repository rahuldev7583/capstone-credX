use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token::TokenAccount};

use crate::{CollateralVault, LoanAccount};

#[derive(Accounts)]
pub struct InitializeLoan<'info>{
    #[account(mut)]
    user: Signer<'info>,
    
    #[account(mut, mint::decimals = 6, mint::authority = mint_authority)]    
    pub credit_mint: Account<'info, Mint>,

    #[account(init_if_needed, payer = user, associated_token::mint = credit_mint, associated_token::authority = user)]
    user_credit_ata: Account<'info, TokenAccount>,

    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(init, payer = user, space = 8 + CollateralVault::INIT_SPACE, seeds = [b"collateral_vault", user.key().as_ref()], bump)]
    collateral_vault: Account<'info, CollateralVault>,

    #[account(init, payer = user, space = LoanAccount::INIT_SPACE, seeds = [b"loan", user.key().as_ref(), collateral_vault.key().as_ref()], bump)]
    loan_account: Account<'info, LoanAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl <'info> InitializeLoan<'info> {
    pub fn initialize_loan(&mut self, collateral_amount: u64, bumps: &InitializeLoanBumps)-> Result<()>{
        self.collateral_vault.set_inner(CollateralVault { mint: self.credit_mint.key(), bump: bumps.collateral_vault});

        self.loan_account.set_inner(LoanAccount { user: self.user.key(), collateral_amount: collateral_amount, remaining_debt: 0, yield_earned: 0, bump: bumps.loan_account});


        Ok(())
    }
}