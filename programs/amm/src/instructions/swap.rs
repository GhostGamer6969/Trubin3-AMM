use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};
use constant_product_curve::ConstantProduct;

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds =[b"config",config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Swap<'info> {
    pub fn swap(&mut self, x_to_y: bool, amount_in: u64, slippage: u16) -> Result<()> {
        require!(self.config.locked == false, AmmError::PoolLocked);
        require!(amount_in != 0, AmmError::InvalidAmount);

        let (user_src, user_dst, vault_src, vault_dst) = if x_to_y {
            (&self.user_x, &self.user_y, &self.vault_y, &self.vault_x)
        } else {
            (&self.user_y, &self.user_x, &self.vault_x, &self.vault_y)
        };

        require!(user_src.amount >= amount_in, AmmError::InsufficientBalance);
        require!(
            vault_src.amount > 0 && vault_dst.amount > 0,
            AmmError::NoLiquidityInPool
        );

        let amount_in_with_fee = (amount_in as u128 * (10_000 - self.config.fee as u128)) / 10_000;

        let amount_out = ConstantProduct::delta_x_from_y_swap_amount(
            vault_dst.amount,
            vault_src.amount,
            amount_in_with_fee as u64,
        )
        .unwrap();

        require!(amount_out != 0, AmmError::InvalidAmount);

        require!(
            vault_dst.amount >= amount_out,
            AmmError::LiquidityLessThanMinimum
        );

        let min_amount_out = (amount_in_with_fee * (10_000 - slippage as u128)) / 10_000;
        require!(
            amount_out as u128 >= min_amount_out,
            AmmError::SlippageExceeded
        );

        self.to_vault(user_src, vault_dst, amount_in)?;
        self.to_user(user_dst, vault_src, amount_out)
    }

    pub fn to_vault(
        &self,
        user: &Account<'info, TokenAccount>,
        vault: &Account<'info, TokenAccount>,
        amount: u64,
    ) -> Result<()> {
        let cpi_accounts = Transfer {
            to: vault.to_account_info(),
            from: user.to_account_info(),
            authority: self.user.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer(cpi_ctx, amount)
    }

    pub fn to_user(
        &self,
        user: &Account<'info, TokenAccount>,
        vault: &Account<'info, TokenAccount>,
        amount: u64,
    ) -> Result<()> {
        let cpi_accounts = Transfer {
            to: user.to_account_info(),
            from: vault.to_account_info(),
            authority: self.config.to_account_info(),
        };

        let seeds = &[
            &b"config"[..],
            &self.config.seed.to_le_bytes(),
            &[self.config.config_bump],
        ];
        let signer_seed = &[&seeds[..]];
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seed);

        transfer(cpi_ctx, amount)
    }
}
