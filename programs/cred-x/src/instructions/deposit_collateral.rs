use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Transfer, Token, TokenAccount};

#[derive(Accounts)]
pub struct DepositCollateral<'info>{
    #[account(mut)]
    user: Signer<'info>,
    
    #[account(mut, mint::decimals = 6, mint::authority = mint_authority)]    
    pub credit_mint: Account<'info, Mint>,

    #[account(mut, associated_token::mint = credit_mint, associated_token::authority = user)]
    user_credit_ata: Account<'info, TokenAccount>,

    #[account(seeds = [b"mint_authority"], bump)]
    pub mint_authority: UncheckedAccount<'info>,

    #[account(mut, seeds = [b"collateral_vault", user.key().as_ref()], bump)]
    collateral_vault: Account<'info, CollateralVault>,

    #[account(init_if_needed, payer = user, associated_token::mint = credit_mint, associated_token::authority = collateral_vault)]
    collateral_vault_ata: Account<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl <'info> DepositCollateral<'info> {
    pub fn deposit_collateral(&mut self, amount: u64)-> Result<()>{
        let program = self.token_program.to_account_info();
        let accounts = Transfer{
            from: self.user_credit_ata.to_account_info(),
            to: self.collateral_vault_ata.to_account_info(),
            authority: self.user.to_account_info()
        };

        let ctx = CpiContext::new(program, accounts);

        transfer(ctx, amount);
        
        Ok(())
    }
}