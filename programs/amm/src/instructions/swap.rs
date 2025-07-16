use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, transfer_checked, Mint, Token, TokenAccount, Transfer, TransferChecked},
};
use constant_product_curve::{ConstantProduct, LiquidityPair};

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        seeds = [b"lp",config.key().as_ref()],
        bump=config.lp_bump,
        mint::decimals = 6,
        mint::authority = config,
    )]
    pub mint_lp: Account<'info, Mint>,
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
    pub fn swap(&mut self, is_x: bool, amount: u64, min: u64) -> Result<()> {
        require!(self.config.locked == false, AmmError::PoolLocked);
        require!(amount != 0, AmmError::InvalidAmount);

        let mut curve = ConstantProduct::init(
            self.vault_x.amount,
            self.vault_y.amount,
            self.mint_lp.supply,
            self.config.fee,
            None,
        )
        .map_err(AmmError::from)?;

        let p = match is_x {
            true => LiquidityPair::X,
            false => LiquidityPair::Y,
        };

        let result = curve.swap(p, amount, min).map_err(AmmError::from)?;

        self.deposit_token(is_x, result.deposit)?;
        self.withdraw_token(is_x, result.withdraw)
    }

    pub fn deposit_token(&mut self, is_x: bool, amount: u64) -> Result<()> {
        let (mint, from, to) = match is_x {
            true => (
                self.mint_x.to_account_info(),
                self.user_x.to_account_info(),
                self.vault_x.to_account_info(),
            ),
            false => (
                self.mint_y.to_account_info(),
                self.user_y.to_account_info(),
                self.vault_y.to_account_info(),
            ),
        };

        let account = TransferChecked {
            from,
            mint,
            to,
            authority: self.user.to_account_info(),
        };
        let ctx = CpiContext::new(self.token_program.to_account_info(), account);
        transfer_checked(ctx, amount, 6)
    }

    pub fn withdraw_token(&mut self, is_x: bool, amount: u64) -> Result<()> {
        let (mint, from, to) = match is_x {
            true => (
                self.mint_x.to_account_info(),
                self.vault_y.to_account_info(),
                self.user_y.to_account_info(),
            ),
            false => (
                self.mint_y.to_account_info(),
                self.vault_x.to_account_info(),
                self.user_x.to_account_info(),
            ),
        };

        let account = TransferChecked {
            from,
            mint,
            to,
            authority: self.config.to_account_info(),
        };

        // [b"config",config.seed.to_le_bytes().as_ref()]
        let seeds = &[
            &b"config"[..],
            &self.config.seed.to_le_bytes(),
            &[self.config.config_bump],
        ];

        let signer_seeds = &[&seeds[..]];

        let ctx = CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            account,
            signer_seeds,
        );
        transfer_checked(ctx, amount, 6)
        // todo!()
    }
}
