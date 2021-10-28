///A Solana version of the xSushi contract
/// https://github.com/sushiswap/sushiswap/blob/master/contracts/SushiBar.sol
/// One notable difference: Given the way that accounts in Solana work,
/// this program is able to create an spl xToken and backing vault for any spl token

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Mint, TokenAccount};
use std::convert::TryInto;

declare_id!("TesTthw7hzgLpsocLDQXS8uH7NMbqjWSvFKM7AWy1zi");

#[program]
pub mod step_staking {
    use super::*;

    pub fn initialize_x_mint(_ctx: Context<InitializeXMint>) -> ProgramResult {
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, mint_bump: u8, amount: u64) -> ProgramResult {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.x_token_mint.supply;
        let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        //mint x tokens
        if total_token == 0 || total_x_token == 0 {
            //no math reqd, we mint them the amount they sent us
            mint_to(
                &ctx.accounts.token_program.to_account_info(),
                &ctx.accounts.x_token_mint.to_account_info(),
                &ctx.accounts.x_token_to.to_account_info(),
                &ctx.accounts.token_mint.to_account_info(),
                mint_bump,
                amount,
            )?;
        } else {
            let what: u64 = 
                (amount as u128).checked_mul(total_x_token as u128).unwrap()
                                .checked_div(total_token as u128).unwrap()
                                .try_into().unwrap();
            mint_to(
                &ctx.accounts.token_program.to_account_info(),
                &ctx.accounts.x_token_mint.to_account_info(),
                &ctx.accounts.x_token_to.to_account_info(),
                &ctx.accounts.token_mint.to_account_info(),
                mint_bump,
                what,
            )?;
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

        (&mut ctx.accounts.token_vault).reload()?;
        (&mut ctx.accounts.x_token_mint).reload()?;
        
        let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        emit!(PriceChange {
            old_step_per_xstep_e9: old_price.0,
            old_step_per_xstep: old_price.1,
            new_step_per_xstep_e9: new_price.0,
            new_step_per_xstep: new_price.1,
        });

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, vault_bump: u8, amount: u64) -> ProgramResult {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.x_token_mint.supply;
        let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

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
            b"vault",
            token_mint_key.as_ref(),
            &[vault_bump],
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

        (&mut ctx.accounts.token_vault).reload()?;
        (&mut ctx.accounts.x_token_mint).reload()?;
        
        let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        emit!(PriceChange {
            old_step_per_xstep_e9: old_price.0,
            old_step_per_xstep: old_price.1,
            new_step_per_xstep_e9: new_price.0,
            new_step_per_xstep: new_price.1,
        });

        Ok(())
    }

    pub fn emit_price(ctx: Context<EmitPrice>) -> ProgramResult {
        let price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);
        emit!(Price {
            step_per_xstep_e9: price.0,
            step_per_xstep: price.1,
        });
        Ok(())
    }

}

fn mint_to<'info>(
    token_program: &AccountInfo<'info>,
    mint: &AccountInfo<'info>, 
    to: &AccountInfo<'info>, 
    seed_account: &AccountInfo<'info>, 
    mint_bump: u8,
    amount: u64,
) -> ProgramResult {
    //compute x_token authority mint seeds
    //note, the x_token authority is itself :mindblown:
    let token_mint_key = seed_account.key();
    let seeds = &[
        b"mint",
        token_mint_key.as_ref(),
        &[mint_bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(
        token_program.clone(),
        token::MintTo {
            mint: mint.clone(),
            to: to.clone(),
            authority: mint.clone(), 
        },
        signer,
    );
    token::mint_to(cpi_ctx, amount)?;

    Ok(())
}

pub fn get_price<'info>(vault: &Account<'info, TokenAccount>, mint: &Account<'info, Mint>) -> (u64, String) {
    let total_token = vault.amount;
    let total_x_token = mint.supply;

    if total_x_token == 0 { 
        return (0, String::from("0"))
    }

    let price_uint = (total_token as u128)
        .checked_mul((10 as u64).pow(mint.decimals as u32) as u128).unwrap()
        .checked_div(total_x_token as u128).unwrap()
        .try_into().unwrap();
    let price_float = (total_token as f64) / (total_x_token as f64);
    return (price_uint, price_float.to_string());
}

#[derive(Accounts)]
pub struct InitializeXMint<'info> {
    ///the token mint to create an xToken for
    pub token_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = initializer,
        mint::decimals = token_mint.decimals,
        mint::authority = x_token_mint, //the PDA address is both the mint account and the authority
        seeds = [ "mint".as_ref(), token_mint.key().as_ref() ],
        bump,
    )]
    ///the not-yet-created, derived xtoken mint pubkey
    pub x_token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = initializer,
        token::mint = token_mint,
        token::authority = token_vault, //the PDA address is both the vault account and the authority
        seeds = [ "vault".as_ref(), token_mint.key().as_ref() ],
        bump,
    )]
    ///the not-yet-created, derived token vault pubkey
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
    )]
    ///pays rent on the initializing accounts
    pub initializer: Signer<'info>,

    ///used by anchor for init of the above
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(mint_bump: u8)]
pub struct Stake<'info> {
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [ b"mint", token_mint.key().as_ref() ],
        bump = mint_bump,
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
        seeds = [ b"vault", token_mint.key().as_ref() ],
        bump,
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
#[instruction(vault_bump: u8)]
pub struct Unstake<'info> {
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [ b"mint", token_mint.key().as_ref() ],
        bump,
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
        seeds = [ b"vault", token_mint.key().as_ref() ],
        bump = vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
    )]
    //the token account to send token
    pub token_to: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct EmitPrice<'info> {
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [ b"mint", token_mint.key().as_ref() ],
        bump,
    )]
    pub x_token_mint: Account<'info, Mint>,

    #[account(
        seeds = [ b"vault", token_mint.key().as_ref() ],
        bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,
}

#[event]
pub struct PriceChange {
    pub old_step_per_xstep_e9: u64,
    pub old_step_per_xstep: String,
    pub new_step_per_xstep_e9: u64,
    pub new_step_per_xstep: String,
}

#[event]
pub struct Price {
    pub step_per_xstep_e9: u64,
    pub step_per_xstep: String,
}