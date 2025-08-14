use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{approve, transfer, Approve, Mint, Token, TokenAccount, Transfer},
};

use crate::{error::CredXError, supported_collateral, CollateralVault, LoanAccount, ProtocolState};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct DepositCollateral<'info> {
    #[account(mut)]
    user: Signer<'info>,
    #[account(
        seeds = [b"protocol", protocol.admin.as_ref()],
        bump = protocol.bump,
        constraint = !protocol.is_locked @ CredXError::ProtocolLocked
    )]
    pub protocol: Account<'info, ProtocolState>,

    #[account(
        constraint = supported_collateral(&collateral_mint.key()) @ CredXError::UnsupportedCollateralMint
    )]
    pub collateral_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = collateral_mint,
        associated_token::authority = user,
        constraint = user_collateral_ata.amount >= amount @ CredXError::InsufficientBalance
    )]
    pub user_collateral_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"collateral_vault", user.key().as_ref()],
        bump,
        constraint = collateral_vault.mint == collateral_mint.key() @ CredXError::MintMismatch
    )]
    pub collateral_vault: Account<'info, CollateralVault>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = collateral_mint,
        associated_token::authority = collateral_vault
    )]
    pub collateral_vault_ata: Account<'info, TokenAccount>,

    /// CHECK: This is a PDA derived from seeds, used as program authority for various operations
    #[account(seeds = [b"program_authority"], bump)]
    pub program_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"loan", user.key().as_ref(), collateral_vault.key().as_ref()],
        bump,
        constraint = loan_account.user == user.key() @ CredXError::UnauthorizedUser
    )]
    pub loan_account: Account<'info, LoanAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> DepositCollateral<'info> {
    pub fn deposit_collateral(&mut self, amount: u64) -> Result<()> {
        require!(amount > 0, CredXError::InvalidAmount);

        require!(
            self.user_collateral_ata.amount >= amount,
            CredXError::InsufficientBalance
        );
        require!(
            self.loan_account.user == self.user.key(),
            CredXError::UnauthorizedUser
        );

        let program = self.token_program.to_account_info();
        let accounts = Transfer {
            from: self.user_collateral_ata.to_account_info(),
            to: self.collateral_vault_ata.to_account_info(),
            authority: self.user.to_account_info(),
        };

        let ctx = CpiContext::new(program, accounts);

        transfer(ctx, amount)?;

        let approve_accounts = Approve {
            to: self.collateral_vault_ata.to_account_info(),
            delegate: self.program_authority.to_account_info(),
            authority: self.collateral_vault.to_account_info(),
        };

        let binding = self.user.key();
        let vault_seeds = &[
            b"collateral_vault",
            binding.as_ref(),
            &[self.collateral_vault.bump],
        ];
        let vault_signer = &[&vault_seeds[..]];

        let approve_ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            approve_accounts,
            vault_signer,
        );

        approve(approve_ctx, amount)?;

        self.loan_account.collateral_amount = self
            .loan_account
            .collateral_amount
            .checked_add(amount)
            .ok_or(CredXError::MathOverflow)?;

        msg!(
            "Deposited {} collateral tokens for user: {}",
            amount,
            self.user.key()
        );

        Ok(())
    }
}
