use anchor_lang::{
    prelude::{Context, *},
    solana_program::{instruction::Instruction, program},
    InstructionData,
};

use crate::{
    gen_signer_seeds,
    handlers::{
        handler_init_farms_for_reserve::InitFarmsForReserve,
        handler_init_obligation_farms_for_reserve::InitObligationFarmsForReserve,
        handler_refresh_obligation_farms_for_reserve::RefreshObligationFarmsForReserve,
    },
    Reserve, ReserveFarmKind,
};

pub fn cpi_initialize_farm_delegated(ctx: &Context<InitFarmsForReserve>) -> Result<()> {
    let lending_market = ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let farm_state_key = ctx.accounts.farm_state.to_account_info().key();
    let accounts = farms::accounts::InitializeFarmDelegated {
        farm_admin: ctx.accounts.lending_market_owner.to_account_info().key(),
        farm_state: farm_state_key,
        farm_delegate: ctx
            .accounts
            .lending_market_authority
            .to_account_info()
            .key(),
        farm_vaults_authority: ctx.accounts.farms_vault_authority.to_account_info().key(),
        global_config: ctx.accounts.farms_global_config.to_account_info().key(),
        system_program: ctx.accounts.system_program.to_account_info().key(),
        rent: ctx.accounts.rent.to_account_info().key(),
    }
    .to_account_metas(None);

    let data = farms::instruction::InitializeFarmDelegated {}.data();

    let instruction = Instruction {
        program_id: ctx.accounts.farms_program.key(),
        accounts,
        data,
    };

    let lending_market_authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

    let account_infos = ctx.accounts.to_account_infos();

    program::invoke_signed(
        &instruction,
        &account_infos,
        &[lending_market_authority_signer_seeds],
    )
    .map_err(Into::into)
}

pub fn cpi_initialize_farmer_delegated(
    ctx: &Context<InitObligationFarmsForReserve>,
    obligation: &Pubkey,
    farm: Pubkey,
) -> Result<()> {
    let lending_market = ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let farmer = ctx.accounts.obligation_farm.to_account_info().key();

    let accounts = farms::accounts::InitializeUser {
        authority: ctx
            .accounts
            .lending_market_authority
            .to_account_info()
            .key(),
        payer: ctx.accounts.payer.key(),
        user_state: farmer,
        farm_state: farm,
        owner: ctx.accounts.owner.key(),
        delegatee: *obligation,
        system_program: ctx.accounts.system_program.to_account_info().key(),
        rent: ctx.accounts.rent.to_account_info().key(),
    }
    .to_account_metas(None);

    let data = farms::instruction::InitializeUser {}.data();

    let instruction = Instruction {
        program_id: ctx.accounts.farms_program.key(),
        accounts,
        data,
    };

    let lending_market_authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

    let account_infos = ctx.accounts.to_account_infos();

    program::invoke_signed(
        &instruction,
        &account_infos,
        &[lending_market_authority_signer_seeds],
    )
    .map_err(Into::into)
}

pub fn cpi_set_stake_delegated(
    ctx: &Context<RefreshObligationFarmsForReserve>,
    reserve: &Reserve,
    mode: ReserveFarmKind,
    amount: u64,
) -> Result<()> {
    let lending_market = ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let farm = reserve.get_farm(mode);
    let farmer = ctx.accounts.obligation_farm_user_state.key();

    let accounts = farms::accounts::SetStakeDelegated {
        delegate_authority: ctx
            .accounts
            .lending_market_authority
            .to_account_info()
            .key(),
        user_state: farmer,
        farm_state: farm,
    }
    .to_account_metas(None);

    let data = farms::instruction::SetStakeDelegated { new_amount: amount }.data();

    let instruction = Instruction {
        program_id: ctx.accounts.farms_program.key(),
        accounts,
        data,
    };

    let lending_market_authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

    let account_infos = ctx.accounts.to_account_infos();

    program::invoke_signed(
        &instruction,
        &account_infos,
        &[lending_market_authority_signer_seeds],
    )
    .map_err(Into::into)
}
