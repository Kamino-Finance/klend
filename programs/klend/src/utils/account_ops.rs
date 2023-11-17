use anchor_lang::{
    prelude::{AccountLoader, Signer},
    AccountsClose, Owner, Result, ToAccountInfo, ZeroCopy,
};
use solana_program::msg;

pub fn close_account_loader<'info, T: ZeroCopy + Owner>(
    close_account: bool,
    owner: &Signer<'info>,
    account_to_be_closed: &AccountLoader<'info, T>,
) -> Result<()> {
    if close_account {
        msg!("Closing account");
        account_to_be_closed.close(owner.to_account_info().clone())?;
    }

    Ok(())
}
