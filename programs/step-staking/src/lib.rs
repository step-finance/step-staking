///A Solana version of the xSushi contract for STEP
/// https://github.com/sushiswap/sushiswap/blob/master/contracts/SushiBar.sol

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Mint, TokenAccount};
use std::convert::TryInto;

#[cfg(feature = "production")]
declare_id!("UNKNOWN" fail build );
#[cfg(not(feature = "production"))]
declare_id!("STak7gf65TjoPaJvttZvinbgLy3vMMsB1ikDx1bK2mH");

#[cfg(feature = "production")]
const STEP_TOKEN_MINT_PUBKEY: &str = "StepAscQoEioFxxWGnh2sLBDFp9d8rvKz2Yp39iDpyT";
#[cfg(not(feature = "production"))]
const STEP_TOKEN_MINT_PUBKEY: &str = "sTEPVXgcctP7rJvoNk8p2Xmo1YrMbcMfu4tgHnowtFm";

#[cfg(feature = "production")]
const X_STEP_TOKEN_MINT_PUBKEY: &str = "xStpgUCss9piqeFUk2iLVcvJEGhAdJxJQuwLkXP555G";
#[cfg(not(feature = "production"))]
const X_STEP_TOKEN_MINT_PUBKEY: &str = "xsTPvEj7rELYcqe2D1k3M5zRe85xWWFK3x1SWDN5qPY";

#[program]
pub mod step_staking {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>, nonce: u8) -> ProgramResult {
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, nonce: u8, amount: u64) -> ProgramResult {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.x_token_mint.supply;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[
            token_mint_key.as_ref(),
            &[nonce],
        ];
        let signer = [&seeds[..]];

        //mint x tokens
        if total_token == 0 && total_x_token == 0 {
            //no math reqd, we mint them the amount they sent us
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.x_token_mint.to_account_info(),
                    to: ctx.accounts.x_token_to.to_account_info(),
                    authority: ctx.accounts.token_vault.to_account_info(), 
                },
                &signer,
            );
            token::mint_to(cpi_ctx, amount)?;
        } else {
            let what: u64 = 
                (amount as u128).checked_mul(total_x_token as u128).unwrap()
                                .checked_div(total_token as u128).unwrap()
                                .try_into().unwrap();
                                
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.x_token_mint.to_account_info(),
                    to: ctx.accounts.x_token_to.to_account_info(),
                    authority: ctx.accounts.token_vault.to_account_info(), 
                },
                &signer,
            );
            token::mint_to(cpi_ctx, what)?;
        }

        //transfer the users tokens to the vault
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.token_from.to_account_info(),
                to: ctx.accounts.token_vault.to_account_info(),
                authority: ctx.accounts.token_from_authority.to_account_info(),
            },
        );
        token::transfer(cpi_ctx, amount)?;

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, nonce: u8, amount: u64) -> ProgramResult {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.x_token_mint.supply;

        //burn what is being sent
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Burn {
                mint: ctx.accounts.x_token_mint.to_account_info(),
                to: ctx.accounts.x_token_from.to_account_info(),
                authority: ctx.accounts.x_token_from_authority.to_account_info(),
            },
        );
        token::burn(cpi_ctx, amount)?;
        
        //determine user share of vault
        let what: u64 = 
            (amount as u128).checked_mul(total_token as u128).unwrap()
                            .checked_div(total_x_token as u128).unwrap()
                            .try_into().unwrap();
                            
        //compute vault signer seeds
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[
            token_mint_key.as_ref(),
            &[nonce],
        ];
        let signer = &[&seeds[..]];

        //transfer from vault to user
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.token_to.to_account_info(),
                authority: ctx.accounts.token_vault.to_account_info(),
            },
            signer,
        );
        token::transfer(cpi_ctx, what)?;
        
        Ok(())
    }

}

#[derive(Accounts)]
#[instruction(nonce: u8)]
pub struct Initialize<'info> {
    #[account(
        address = STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = initializer,
        token::mint = token_mint,
        token::authority = token_vault, //the PDA address is both the vault account and the authority (and event the mint authority)
        seeds = [ STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap().as_ref() ],
        bump = nonce,
    )]
    ///the not-yet-created, derived token vault pubkey
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
    )]
    ///pays rent on the initializing accounts
    pub initializer: Signer<'info>,

    ///used by anchor for init of the token
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(nonce: u8)]
pub struct Stake<'info> {
    #[account(
        address = STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        address = X_STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub x_token_mint: Account<'info, Mint>,

    #[account(
        mut,
    )]
    //the token account to withdraw from
    pub token_from: Account<'info, TokenAccount>,

    //the authority allowed to transfer from token_from
    pub token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = nonce,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
    )]
    //the token account to send xtoken
    pub x_token_to: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(nonce: u8)]
pub struct Unstake<'info> {
    #[account(
        address = STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        address = X_STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub x_token_mint: Account<'info, Mint>,

    #[account(
        mut,
    )]
    //the token account to withdraw from
    pub x_token_from: Account<'info, TokenAccount>,

    //the authority allowed to transfer from x_token_from
    pub x_token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = nonce,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
    )]
    //the token account to send token
    pub token_to: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}