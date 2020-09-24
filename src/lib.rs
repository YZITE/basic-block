#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![recursion_limit = "100"]

extern crate alloc;
extern crate core;

mod arena;
mod bb;
pub mod jump;

pub use arena::{Arena, InvalidId, OffendingIds, SetBbLabelError};
pub use bb::{BasicBlock, BasicBlockInner};
pub type BbId = usize;
pub type Label = alloc::borrow::Cow<'static, str>;
