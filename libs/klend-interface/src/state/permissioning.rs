use bitflags::bitflags;

bitflags! {
    /// Mirror of the on-chain `PermissionedOp` bitmask
    /// (see `programs/klend/src/utils/permissioning.rs`).
    ///
    /// Stored as a raw `u64` on `LendingMarket.permissioned_ops` and
    /// `ReserveConfig.permissioned_ops`. Bit values must stay in lockstep
    /// with the program; the `permissioned_op_bits` test in
    /// `tests/alignment.rs` pins them.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct PermissionedOp: u64 {
        const DEPOSIT   = 1 << 0;
        const BORROW    = 1 << 1;
        const LIQUIDATE = 1 << 2;
        // REPAY and WITHDRAW are never permissioned.

        const NONE = 0;
    }
}
