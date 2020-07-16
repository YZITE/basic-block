use std::iter;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditional<C, T> {
    pub target: T,
    pub condition: C,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unconditional<T> {
    Halt,
    Jump(T),
    Return,
    Unknown,
}

pub trait ForeachTarget {
    type JumpTarget;

    fn foreach_target<F>(&self, f: F)
    where
        F: FnMut(&Self::JumpTarget);

    fn foreach_target_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::JumpTarget);
}

impl<C, T> ForeachTarget for Conditional<C, T> {
    type JumpTarget = T;

    #[inline]
    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        f(&self.target);
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        f(&mut self.target)
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
