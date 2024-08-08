use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::Mint;

use crate::{
    state::{
        obligation::{Obligation, ObligationCollateral, ObligationLiquidity},
        LendingMarket,
    },
    utils::{consts::OBLIGATION_SIZE, seeds::BASE_SEED_USER_METADATA},
    InitObligationArgs, LendingError, UserMetadata,
};

pub fn process(ctx: Context<InitObligation>, args: InitObligationArgs) -> Result<()> {
    let clock = &Clock::get()?;

    require!(args.id == 0, LendingError::InvalidObligationId);

    check_obligation_seeds(
        args.tag,
        &ctx.accounts.seed1_account,
        &ctx.accounts.seed2_account,
    )
    .unwrap();

    let obligation = &mut ctx.accounts.obligation.load_init()?;
    let owner_user_metadata = &ctx.accounts.owner_user_metadata.load()?;

    obligation.init(crate::state::obligation::InitObligationParams {
        current_slot: clock.slot,
        lending_market: ctx.accounts.lending_market.key(),
        owner: ctx.accounts.obligation_owner.key(),
        deposits: [ObligationCollateral::default(); 8],
        borrows: [ObligationLiquidity::default(); 5],
        tag: args.tag as u64,
        referrer: owner_user_metadata.referrer,
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: InitObligationArgs)]
pub struct InitObligation<'info> {
    pub obligation_owner: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(init,
        seeds = [&[args.tag], &[args.id], obligation_owner.key().as_ref(), lending_market.key().as_ref(), seed1_account.key().as_ref(), seed2_account.key().as_ref()],
        bump,
        payer = fee_payer,
        space = OBLIGATION_SIZE + 8,
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    pub seed1_account: AccountInfo<'info>,
    pub seed2_account: AccountInfo<'info>,

    #[account(
        seeds = [BASE_SEED_USER_METADATA, obligation_owner.key().as_ref()],
        bump = owner_user_metadata.load()?.bump.try_into().unwrap(),
    )]
    pub owner_user_metadata: AccountLoader<'info, UserMetadata>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

pub fn check_obligation_seeds(
    tag: u8,
    seed1_account: &AccountInfo,
    seed2_account: &AccountInfo,
) -> Result<()> {
    let seed1_key = seed1_account.key();
    let seed2_key = seed2_account.key();
    match tag {
        0 => {
            require!(
                seed1_key == Pubkey::default() && seed2_key == Pubkey::default(),
                LendingError::InvalidObligationSeedsValue
            );
        }
        1 => {
            let _mint1_check =
                Mint::try_deserialize(&mut seed1_account.data.borrow().as_ref()).unwrap();
            let _mint2_check =
                Mint::try_deserialize(&mut seed2_account.data.borrow().as_ref()).unwrap();
        }
        2 => {
            let _mint_check =
                Mint::try_deserialize(&mut seed1_account.data.borrow().as_ref()).unwrap();
            require!(
                seed1_key == seed2_key,
                LendingError::InvalidObligationSeedsValue
            )
        }
        3 => {
            let _mint1_check =
                Mint::try_deserialize(&mut seed1_account.data.borrow().as_ref()).unwrap();
            let _mint2_check =
                Mint::try_deserialize(&mut seed2_account.data.borrow().as_ref()).unwrap();
        }
        _ => {}
    }

    Ok(())
}
