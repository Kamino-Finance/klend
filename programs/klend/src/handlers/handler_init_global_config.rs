use anchor_lang::{prelude::*, Accounts};

use crate::{
    state::GlobalConfig,
    utils::{
        seeds::{self, pda},
        GLOBAL_CONFIG_SIZE,
    },
    LendingError,
};

#[cfg(not(feature = "idl-build"))]
pub fn process(ctx: Context<InitGlobalConfig>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_init()?;
    global_config.init(
        ctx.accounts
            .program_data
            .upgrade_authority_address
            .ok_or(LendingError::NoUpgradeAuthority)?,
    );

    Ok(())
}

#[cfg(feature = "idl-build")]
pub fn process(_ctx: Context<InitGlobalConfig>) -> Result<()> {
    panic!("This instruction is not available in idl-build mode");
}

#[derive(Accounts)]
pub struct InitGlobalConfig<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + GLOBAL_CONFIG_SIZE,
        seeds = [seeds::GLOBAL_CONFIG_STATE],
        bump,
    )]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account(
        address = pda::program_data(),
    )]
    #[cfg(not(feature = "idl-build"))]
    pub program_data: Account<'info, ProgramData>,

    #[cfg(feature = "idl-build")]
    pub program_data: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
