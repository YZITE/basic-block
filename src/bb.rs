use crate::jump::{self, ForeachTarget};
use alloc::vec::Vec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum BasicBlockInner<S, C, T> {
    Concrete {
        statements: Vec<S>,
        condjmp: Option<C>,
        next: jump::Unconditional<T>,
    },
    /// placeholder for linker references to other files
    Placeholder { is_extern: bool },
}

impl<S, C, T> Default for BasicBlockInner<S, C, T> {
    #[inline]
    fn default() -> Self {
        Self::Placeholder { is_extern: false }
    }
}

impl<S, C, T> BasicBlockInner<S, C, T> {
    #[inline]
    pub fn is_concrete(&self) -> bool {
        if let Self::Concrete { .. } = self {
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn is_placeholder(&self) -> bool {
        if let Self::Placeholder { .. } = self {
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct BasicBlock<S, C, T> {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub inner: BasicBlockInner<S, C, T>,
    pub is_public: bool,
}

impl<S, C, T> ForeachTarget for BasicBlockInner<S, C, T>
where
    S: ForeachTarget<JumpTarget = T>,
    C: ForeachTarget<JumpTarget = T>,
{
    type JumpTarget = T;

    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        if let BasicBlockInner::Concrete {
            statements,
            condjmp,
            next,
        } = self
        {
            statements.foreach_target(&mut f);
            condjmp.foreach_target(&mut f);
            next.foreach_target(f);
        }
    }

    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        if let BasicBlockInner::Concrete {
            statements,
            condjmp,
            next,
        } = self
        {
            statements.foreach_target_mut(&mut f);
            condjmp.foreach_target_mut(&mut f);
            next.foreach_target_mut(f);
        }
    }
}

impl<S, C, T> ForeachTarget for BasicBlock<S, C, T>
where
    S: ForeachTarget<JumpTarget = T>,
    C: ForeachTarget<JumpTarget = T>,
{
    type JumpTarget = T;

    #[inline(always)]
    fn foreach_target<F>(&self, f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        self.inner.foreach_target(f);
    }

    #[inline(always)]
    fn foreach_target_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        self.inner.foreach_target_mut(f);
    }
}
