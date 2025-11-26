use anchor_lang::prelude::*;

use crate::BorrowOrder;

#[event]
pub struct BorrowOrderPlaceEvent {
    pub after: BorrowOrder,
}

#[event]
pub struct BorrowOrderUpdateEvent {
    pub before: BorrowOrder,
    pub after: BorrowOrder,
}

#[event]
pub struct BorrowOrderCancelEvent {
    pub before: BorrowOrder,
}

#[event]
pub struct BorrowOrderPartialFillEvent {
    pub before: BorrowOrder,
    pub after: BorrowOrder,
}

#[event]
pub struct BorrowOrderFullFillEvent {
    pub before: BorrowOrder,
}
