use anchor_lang::{prelude::*, Event};



pub trait EventEmitter {
    fn emit(&self, event: impl Event) -> Result<()>;
}


macro_rules! ctx_event_emitter {
    ($ctx: expr) => {
        $crate::utils::CpiEventEmitter($crate::utils::EventCtx {
            accounts: $crate::utils::EventCtxAccounts {
                event_authority: $ctx.accounts.event_authority.to_account_info(),
            },
            bumps: $crate::utils::EventCtxBumps {
                event_authority: $ctx.bumps.event_authority,
            },
        })
    };
}
pub(crate) use ctx_event_emitter;



pub struct CpiEventEmitter<'info>(pub EventCtx<'info>);

impl<'info> EventEmitter for CpiEventEmitter<'info> {
    fn emit(&self, event: impl Event) -> Result<()> {
        let ctx = &self.0;
        emit_cpi!(event);
        Ok(())
    }
}

pub struct EventCtx<'info> {
    pub accounts: EventCtxAccounts<'info>,
    pub bumps: EventCtxBumps,
}

pub struct EventCtxAccounts<'info> {
    pub event_authority: AccountInfo<'info>,
}

pub struct EventCtxBumps {
    pub event_authority: u8,
}
