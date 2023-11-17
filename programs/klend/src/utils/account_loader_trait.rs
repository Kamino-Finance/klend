use std::cell::{Ref, RefMut};

use anchor_lang::{prelude::AccountLoader, Key, Owner, Result, ZeroCopy};
use solana_program::pubkey::Pubkey;

pub trait AnyAccountLoader<'info, T> {
    fn get_mut(&self) -> Result<RefMut<T>>;
    fn get(&self) -> Result<Ref<T>>;
    fn get_pubkey(&self) -> Pubkey;
}

impl<'info, T: ZeroCopy + Owner> AnyAccountLoader<'info, T> for AccountLoader<'info, T> {
    fn get_mut(&self) -> Result<RefMut<T>> {
        self.load_mut()
    }
    fn get(&self) -> Result<Ref<T>> {
        self.load()
    }

    fn get_pubkey(&self) -> Pubkey {
        self.key()
    }
}
