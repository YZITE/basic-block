#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;
extern crate core;

mod arena;
mod bb;
pub mod jump;

pub use arena::{Arena, OffendingIds, SetBbLabelError};
pub use bb::{BasicBlock, BasicBlockInner};
pub type BbId = usize;
pub type Label = alloc::borrow::Cow<'static, str>;
