///A Solana version of the xSushi contract for STEP
/// https://github.com/sushiswap/sushiswap/blob/master/contracts/SushiBar.sol
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, spl_token::instruction::AuthorityType, Mint, Token, TokenAccount},
};
use std::convert::TryInto;

#[cfg(not(feature = "test-id"))]
declare_id!("Stk5NCWomVN3itaFjLu382u9ibb5jMSHEsh6CuhaGjB");
#[cfg(feature = "test-id")]
declare_id!("TesT35sGptoswsVkcLpUUe6U2iTJZE59on1Jno8Vdpg");

#[cfg(not(feature = "local-testing"))]
pub mod constants {
    pub const STEP_TOKEN_MINT_PUBKEY: &str = "StepAscQoEioFxxWGnh2sLBDFp9d8rvKz2Yp39iDpyT";
    pub const X_STEP_TOKEN_MINT_PUBKEY: &str = "xStpgUCss9piqeFUk2iLVcvJEGhAdJxJQuwLkXP555G";
}

#[cfg(feature = "local-testing")]
pub mod constants {
    pub const STEP_TOKEN_MINT_PUBKEY: &str = "teST1ieLrLdr4MJPZ7i8mgSCLQ7rTrPRjNnyFdHFaz9";
    pub const X_STEP_TOKEN_MINT_PUBKEY: &str = "TestZ4qmw6fCo1uK9oJbobWDgj1sME6hR1ssWQnyjxM";
}

#[program]
pub mod step_staking {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>, _nonce: u8) -> Result<()> {
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, nonce: u8, amount: u64) -> Result<()> {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.x_token_mint.supply;
        let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[token_mint_key.as_ref(), &[nonce]];
        let signer = [&seeds[..]];

        //mint x tokens
        if total_token == 0 || total_x_token == 0 {
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
            let what: u64 = (amount as u128)
                .checked_mul(total_x_token as u128)
                .unwrap()
                .checked_div(total_token as u128)
                .unwrap()
                .try_into()
                .unwrap();

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

        ctx.accounts.token_vault.reload()?;
        ctx.accounts.x_token_mint.reload()?;

        let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        emit!(PriceChange {
            old_step_per_xstep_e9: old_price.0,
            old_step_per_xstep: old_price.1,
            new_step_per_xstep_e9: new_price.0,
            new_step_per_xstep: new_price.1,
        });

        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, nonce: u8, amount: u64) -> Result<()> {
        let total_token = ctx.accounts.token_vault.amount;
        let total_x_token = ctx.accounts.x_token_mint.supply;
        let old_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        //burn what is being sent
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Burn {
                mint: ctx.accounts.x_token_mint.to_account_info(),
                from: ctx.accounts.x_token_from.to_account_info(),
                authority: ctx.accounts.x_token_from_authority.to_account_info(),
            },
        );
        token::burn(cpi_ctx, amount)?;

        //determine user share of vault
        let what: u64 = (amount as u128)
            .checked_mul(total_token as u128)
            .unwrap()
            .checked_div(total_x_token as u128)
            .unwrap()
            .try_into()
            .unwrap();

        //compute vault signer seeds
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds = &[token_mint_key.as_ref(), &[nonce]];
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

        ctx.accounts.token_vault.reload()?;
        ctx.accounts.x_token_mint.reload()?;

        let new_price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);

        emit!(PriceChange {
            old_step_per_xstep_e9: old_price.0,
            old_step_per_xstep: old_price.1,
            new_step_per_xstep_e9: new_price.0,
            new_step_per_xstep: new_price.1,
        });

        Ok(())
    }

    pub fn emit_price(ctx: Context<EmitPrice>) -> Result<()> {
        let price = get_price(&ctx.accounts.token_vault, &ctx.accounts.x_token_mint);
        emit!(Price {
            step_per_xstep_e9: price.0,
            step_per_xstep: price.1,
        });
        Ok(())
    }
}

const E9: u128 = 1000000000;

pub fn get_price<'info>(
    vault: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
) -> (u64, String) {
    let total_token = vault.amount;
    let total_x_token = mint.supply;

    if total_x_token == 0 {
        return (0, String::from("0"));
    }

    let price_uint = (total_token as u128)
        .checked_mul(E9)
        .unwrap()
        .checked_div(total_x_token as u128)
        .unwrap()
        .try_into()
        .unwrap();
    let price_float = (total_token as f64) / (total_x_token as f64);
    (price_uint, price_float.to_string())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = initializer,
        token::mint = token_mint,
        token::authority = token_vault, //the PDA address is both the vault account and the authority (and event the mint authority)
        seeds = [ constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap().as_ref() ],
        bump,
    )]
    ///the not-yet-created, derived token vault pubkey
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
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
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = constants::X_STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub x_token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    //the token account to withdraw from
    pub token_from: Box<Account<'info, TokenAccount>>,

    //the authority allowed to transfer from token_from
    pub token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = nonce,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    //the token account to send xtoken
    pub x_token_to: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(nonce: u8)]
pub struct Unstake<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = constants::X_STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub x_token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    //the token account to withdraw from
    pub x_token_from: Box<Account<'info, TokenAccount>>,

    //the authority allowed to transfer from x_token_from
    pub x_token_from_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump = nonce,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    //the token account to send token
    pub token_to: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct EmitPrice<'info> {
    #[account(
        address = constants::STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = constants::X_STEP_TOKEN_MINT_PUBKEY.parse::<Pubkey>().unwrap(),
    )]
    pub x_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [ token_mint.key().as_ref() ],
        bump,
    )]
    pub token_vault: Box<Account<'info, TokenAccount>>,
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
