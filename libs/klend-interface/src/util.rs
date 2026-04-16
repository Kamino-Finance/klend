use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;

/// Build an `AccountMeta` for an optional account. When `opt` is `None`, the program ID is
/// substituted (Anchor 0.29 convention for optional accounts).
pub fn optional_account(
    program_id: &Pubkey,
    opt: Option<Pubkey>,
    is_writable: bool,
) -> AccountMeta {
    match opt {
        Some(key) => AccountMeta {
            pubkey: key,
            is_signer: false,
            is_writable,
        },
        None => AccountMeta {
            pubkey: *program_id,
            is_signer: false,
            is_writable: false,
        },
    }
}

pub fn readonly(pubkey: Pubkey) -> AccountMeta {
    AccountMeta {
        pubkey,
        is_signer: false,
        is_writable: false,
    }
}

pub fn writable(pubkey: Pubkey) -> AccountMeta {
    AccountMeta {
        pubkey,
        is_signer: false,
        is_writable: true,
    }
}

pub fn signer(pubkey: Pubkey) -> AccountMeta {
    AccountMeta {
        pubkey,
        is_signer: true,
        is_writable: false,
    }
}

pub fn signer_writable(pubkey: Pubkey) -> AccountMeta {
    AccountMeta {
        pubkey,
        is_signer: true,
        is_writable: true,
    }
}
