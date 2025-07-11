use anchor_lang::{
    err,
    prelude::{AccountLoader, Context},
    Bumps, Result,
};

use crate::{state::LendingMarket, LendingError};

pub fn emergency_mode_disabled(lending_market: &AccountLoader<LendingMarket>) -> Result<()> {
    if lending_market.load()?.emergency_mode > 0 {
        return err!(LendingError::GlobalEmergencyMode);
    }
    Ok(())
}

pub fn check_remaining_accounts<T>(ctx: &Context<T>) -> Result<()>
where
    T: Bumps,
{
    if !ctx.remaining_accounts.is_empty() {
        return err!(LendingError::InvalidAccountInput);
    }

    Ok(())
}

pub mod token_2022 {
    use anchor_lang::err;
    use anchor_spl::{
        token::spl_token,
        token_2022::{
            spl_token_2022, spl_token_2022::extension::confidential_transfer::EncryptedBalance,
        },
        token_interface::spl_token_2022::extension::{
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
    };
    use bytemuck::Zeroable;
    use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

    use crate::{xmsg, LendingError};

    const VALID_LIQUIDITY_TOKEN_EXTENSIONS: &[ExtensionType] = &[
        ExtensionType::ConfidentialTransferFeeConfig,
        ExtensionType::ConfidentialTransferMint,
        ExtensionType::MintCloseAuthority,
        ExtensionType::MetadataPointer,
        ExtensionType::PermanentDelegate,
        ExtensionType::TransferFeeConfig,
        ExtensionType::TokenMetadata,
        ExtensionType::TransferHook,
        ExtensionType::DefaultAccountState,
        ExtensionType::ScaledUiAmount,
        ExtensionType::Pausable,
    ];

    pub fn validate_liquidity_token_extensions(
        mint_acc_info: &AccountInfo,
        token_acc_info: &AccountInfo,
    ) -> anchor_lang::Result<()> {
        if mint_acc_info.owner == &spl_token::id() {
            return Ok(());
        }

        if token_acc_info.owner == &spl_token::id() {
            return err!(LendingError::InvalidTokenAccount);
        }

        let mint_data = mint_acc_info.data.borrow();
        let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

        let token_acc_data = token_acc_info.data.borrow();
        let token_acc =
            StateWithExtensions::<spl_token_2022::state::Account>::unpack(&token_acc_data)?;

        for mint_ext in mint.get_extension_types()? {
            if !VALID_LIQUIDITY_TOKEN_EXTENSIONS.contains(&mint_ext) {
                xmsg!(
                    "Invalid liquidity token (2022) extension: {:?}, supported extensions: {:?}",
                    mint_ext,
                    VALID_LIQUIDITY_TOKEN_EXTENSIONS
                );
                return err!(LendingError::UnsupportedTokenExtension);
            }
            match mint_ext {
                ExtensionType::TransferFeeConfig => {
                    let ext = mint
                        .get_extension::<spl_token_2022::extension::transfer_fee::TransferFeeConfig>()?;
                    if <u16>::from(ext.older_transfer_fee.transfer_fee_basis_points) != 0
                        || <u16>::from(ext.newer_transfer_fee.transfer_fee_basis_points) != 0
                    {
                        xmsg!(
                            "Transfer fee must be 0 for liquidity tokens, got: {:?}",
                            ext
                        );
                        return err!(LendingError::UnsupportedTokenExtension);
                    }
                }
                ExtensionType::TransferHook => {
                    let ext = mint
                        .get_extension::<spl_token_2022::extension::transfer_hook::TransferHook>(
                        )?;
                    let hook_program_id: Option<Pubkey> = ext.program_id.into();
                    if hook_program_id.is_some() {
                        xmsg!(
                            "Transfer hook program id must not be set for liquidity tokens, got {:?}",
                            ext
                        );
                        return err!(LendingError::UnsupportedTokenExtension);
                    }
                }
                ExtensionType::ConfidentialTransferMint => {
                    let ext = mint.get_extension::<spl_token_2022::extension::confidential_transfer::ConfidentialTransferMint>()?;
                    if bool::from(ext.auto_approve_new_accounts) {
                        xmsg!(
                            "Auto approve new accounts must be false for liquidity tokens, got {:?}",
                            ext
                        );
                        return err!(LendingError::UnsupportedTokenExtension);
                    }

                    if let Ok(token_acc_ext) = token_acc.get_extension::<spl_token_2022::extension::confidential_transfer::ConfidentialTransferAccount>() {
                        if bool::from(token_acc_ext.allow_confidential_credits) {
                            xmsg!(
                                "Allow confidential credits must be false for token accounts, got {:?}",
                                token_acc_ext
                            );
                            return err!(LendingError::UnsupportedTokenExtension);
                        }
                        if token_acc_ext.pending_balance_lo != EncryptedBalance::zeroed()
                            && token_acc_ext.pending_balance_hi != EncryptedBalance::zeroed()
                        {
                            xmsg!(
                                "Pending balance must be zero for token accounts, got {:?}",
                                token_acc_ext
                            );
                            return err!(LendingError::UnsupportedTokenExtension);
                        }
                    }
                }
                ExtensionType::DefaultAccountState => {
                    let ext = mint.get_extension::<spl_token_2022::extension::default_account_state::DefaultAccountState>()?;
                    if ext.state != spl_token_2022::state::AccountState::Initialized as u8 {
                        xmsg!(
                            "Default account state extension only supports \"Initialized\" state"
                        );
                        return err!(LendingError::UnsupportedTokenExtension);
                    }
                }
                ExtensionType::Pausable => {
                    let ext = mint
                        .get_extension::<spl_token_2022::extension::pausable::PausableConfig>()?;
                    if ext.paused.into() {
                        xmsg!("Pausable extension must not be paused for liquidity tokens");
                        return err!(LendingError::UnsupportedTokenExtension);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
