use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::info::{ObligationContext, ObligationContextError};

impl ObligationContext {
    /// Borrow liquidity from a reserve against this obligation.
    pub fn borrow(
        &self,
        owner: Pubkey,
        borrow_reserve: &Pubkey,
        user_destination_liquidity: Pubkey,
        amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let r = self
            .find_reserve(borrow_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*borrow_reserve))?;
        let farms = self.debt_farms(borrow_reserve);
        Ok(super::borrow::borrow(
            owner,
            &r.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_destination_liquidity,
            amount,
            farms.as_ref(),
        ))
    }

    /// Deposit liquidity into a reserve and credit collateral to this obligation.
    pub fn deposit(
        &self,
        owner: Pubkey,
        reserve: &Pubkey,
        user_source_liquidity: Pubkey,
        amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let r = self
            .find_reserve(reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*reserve))?;
        let farms = self.collateral_farms(reserve);
        Ok(super::deposit::deposit_to_obligation(
            owner,
            &r.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_source_liquidity,
            amount,
            farms.as_ref(),
        ))
    }

    /// Withdraw collateral from this obligation and redeem it for liquidity.
    pub fn withdraw(
        &self,
        owner: Pubkey,
        reserve: &Pubkey,
        user_destination_liquidity: Pubkey,
        collateral_amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let r = self
            .find_reserve(reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*reserve))?;
        let farms = self.collateral_farms(reserve);
        Ok(super::withdraw::withdraw(
            owner,
            &r.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_destination_liquidity,
            collateral_amount,
            farms.as_ref(),
        ))
    }

    /// Withdraw collateral (cTokens) from this obligation without redeeming.
    pub fn withdraw_collateral(
        &self,
        owner: Pubkey,
        reserve: &Pubkey,
        user_destination_collateral: Pubkey,
        collateral_amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let r = self
            .find_reserve(reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*reserve))?;
        let farms = self.collateral_farms(reserve);
        Ok(super::withdraw::withdraw_collateral(
            owner,
            &r.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_destination_collateral,
            collateral_amount,
            farms.as_ref(),
        ))
    }

    /// Repay borrowed liquidity.
    pub fn repay(
        &self,
        owner: Pubkey,
        reserve: &Pubkey,
        user_source_liquidity: Pubkey,
        amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let r = self
            .find_reserve(reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*reserve))?;
        let farms = self.debt_farms(reserve);
        Ok(super::repay::repay(
            owner,
            &r.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_source_liquidity,
            amount,
            farms.as_ref(),
        ))
    }

    /// Atomically repay a borrow, withdraw collateral, and redeem it for liquidity.
    #[allow(clippy::too_many_arguments)]
    pub fn repay_and_withdraw(
        &self,
        owner: Pubkey,
        repay_reserve: &Pubkey,
        withdraw_reserve: &Pubkey,
        user_source_liquidity: Pubkey,
        user_destination_liquidity: Pubkey,
        repay_amount: u64,
        withdraw_collateral_amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let rr = self
            .find_reserve(repay_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*repay_reserve))?;
        let wr = self
            .find_reserve(withdraw_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*withdraw_reserve))?;
        let collateral_farms = self.collateral_farms(withdraw_reserve);
        let debt_farms = self.debt_farms(repay_reserve);
        Ok(super::repay::repay_and_withdraw(
            owner,
            &rr.info,
            &wr.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_source_liquidity,
            user_destination_liquidity,
            repay_amount,
            withdraw_collateral_amount,
            collateral_farms.as_ref(),
            debt_farms.as_ref(),
        ))
    }

    /// Atomically deposit to one reserve and withdraw from another (rebalancing).
    #[allow(clippy::too_many_arguments)]
    pub fn deposit_and_withdraw(
        &self,
        owner: Pubkey,
        deposit_reserve: &Pubkey,
        withdraw_reserve: &Pubkey,
        user_source_liquidity: Pubkey,
        user_destination_liquidity: Pubkey,
        deposit_amount: u64,
        withdraw_collateral_amount: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let dr = self
            .find_reserve(deposit_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*deposit_reserve))?;
        let wr = self
            .find_reserve(withdraw_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*withdraw_reserve))?;
        let deposit_farms = self.collateral_farms(deposit_reserve);
        let withdraw_farms = self.collateral_farms(withdraw_reserve);
        Ok(super::compound::deposit_and_withdraw(
            owner,
            &dr.info,
            &wr.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_source_liquidity,
            user_destination_liquidity,
            deposit_amount,
            withdraw_collateral_amount,
            deposit_farms.as_ref(),
            withdraw_farms.as_ref(),
        ))
    }

    /// Liquidate an undercollateralized obligation.
    #[allow(clippy::too_many_arguments)]
    pub fn liquidate(
        &self,
        liquidator: Pubkey,
        repay_reserve: &Pubkey,
        withdraw_reserve: &Pubkey,
        user_source_liquidity: Pubkey,
        user_destination_collateral: Pubkey,
        user_destination_liquidity: Pubkey,
        amount: u64,
        min_received: u64,
        max_ltv_override: u64,
    ) -> Result<Vec<Instruction>, ObligationContextError> {
        let rr = self
            .find_reserve(repay_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*repay_reserve))?;
        let wr = self
            .find_reserve(withdraw_reserve)
            .ok_or(ObligationContextError::ReserveNotFound(*withdraw_reserve))?;
        let collateral_farms = self.collateral_farms(withdraw_reserve);
        let debt_farms = self.debt_farms(repay_reserve);
        Ok(super::liquidate::liquidate(
            liquidator,
            &rr.info,
            &wr.info,
            &self.obligation,
            &self.all_reserve_infos(),
            user_source_liquidity,
            user_destination_collateral,
            user_destination_liquidity,
            amount,
            min_received,
            max_ltv_override,
            collateral_farms.as_ref(),
            debt_farms.as_ref(),
        ))
    }

    /// Request an elevation group change for this obligation.
    pub fn request_elevation_group(&self, owner: Pubkey, elevation_group: u8) -> Vec<Instruction> {
        super::obligation::request_elevation_group(
            owner,
            self.lending_market,
            &self.obligation,
            &self.all_reserve_infos(),
            elevation_group,
        )
    }
}
