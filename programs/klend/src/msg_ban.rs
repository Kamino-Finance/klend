






#[allow(unused_macros)]
macro_rules! msg {
    ($($args:tt)*) => {
        compile_error!("plain `msg!` is banned; use `xmsg!` (or the fully-qualified `anchor_lang::prelude::msg!` if the raw macro is genuinely needed)")
    };
}
