use core::iter;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum Unconditional<T> {
    Halt,
    Jump(T),
    Return,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Dummy<T>(pub core::marker::PhantomData<T>);

pub trait ForeachTarget {
    type JumpTarget;

    fn foreach_target<F>(&self, f: F)
    where
        F: FnMut(&Self::JumpTarget);

    fn foreach_target_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::JumpTarget);
}

impl<T> ForeachTarget for Dummy<T> {
    type JumpTarget = T;

    #[inline]
    fn foreach_target<F>(&self, _f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, _f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
    }
}

impl<T> ForeachTarget for Unconditional<T> {
    type JumpTarget = T;

    #[inline]
    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        if let Unconditional::Jump(t) = self {
            f(t);
        }
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        if let Unconditional::Jump(t) = self {
            f(t);
        }
    }
}

impl<C, T> ForeachTarget for C
where
    for<'a> &'a C: iter::IntoIterator<Item = &'a T>,
    for<'a> &'a mut C: iter::IntoIterator<Item = &'a mut T>,
    T: ForeachTarget,
{
    type JumpTarget = T::JumpTarget;

    #[inline]
    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        for i in self {
            i.foreach_target(&mut f);
        }
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        for i in self {
            i.foreach_target_mut(&mut f);
        }
    }
}
