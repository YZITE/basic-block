use crate::jump::{self, IntoTargetsIter};
use alloc::vec::Vec;
use core::iter;

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
    #[inline(always)]
    pub fn is_concrete(&self) -> bool {
        matches!(self, Self::Concrete { .. })
    }

    #[inline(always)]
    pub fn is_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct BasicBlock<S, C, T> {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub inner: BasicBlockInner<S, C, T>,
    pub is_public: bool,
}

type ItiWrap<Sr, Cr, Tr, Sslc, Copt> = iter::Flatten<
    core::option::IntoIter<
        iter::Chain<
            iter::Chain<
                iter::FlatMap<
                    Sslc,
                    <Sr as IntoTargetsIter>::IntoTrgsIter,
                    fn(Sr) -> <Sr as IntoTargetsIter>::IntoTrgsIter,
                >,
                iter::FlatMap<
                    Copt,
                    <Cr as IntoTargetsIter>::IntoTrgsIter,
                    fn(Cr) -> <Cr as IntoTargetsIter>::IntoTrgsIter,
                >,
            >,
            core::option::IntoIter<Tr>,
        >,
    >,
>;

impl<'a, S: 'a, C: 'a, T: 'a> IntoTargetsIter for &'a BasicBlockInner<S, C, T>
where
    &'a S: IntoTargetsIter<Target = &'a T>,
    &'a C: IntoTargetsIter<Target = &'a T>,
{
    type Target = &'a T;
    type IntoTrgsIter =
        ItiWrap<&'a S, &'a C, &'a T, core::slice::Iter<'a, S>, core::option::Iter<'a, C>>;

    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        if let BasicBlockInner::Concrete {
            statements,
            condjmp,
            next,
        } = self
        {
            Some(
                statements
                    .iter()
                    .flat_map(
                        IntoTargetsIter::into_trgs_iter
                            as fn(&'a S) -> <&'a S as IntoTargetsIter>::IntoTrgsIter,
                    )
                    .chain(condjmp.iter().flat_map(
                        IntoTargetsIter::into_trgs_iter
                            as fn(&'a C) -> <&'a C as IntoTargetsIter>::IntoTrgsIter,
                    ))
                    .chain(next.into_trgs_iter()),
            )
        } else {
            None
        }
        .into_iter()
        .flatten()
    }
}

impl<'a, S: 'a, C: 'a, T: 'a> IntoTargetsIter for &'a mut BasicBlockInner<S, C, T>
where
    &'a mut S: IntoTargetsIter<Target = &'a mut T>,
    &'a mut C: IntoTargetsIter<Target = &'a mut T>,
{
    type Target = &'a mut T;
    type IntoTrgsIter = ItiWrap<
        &'a mut S,
        &'a mut C,
        &'a mut T,
        core::slice::IterMut<'a, S>,
        core::option::IterMut<'a, C>,
    >;

    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        if let BasicBlockInner::Concrete {
            statements,
            condjmp,
            next,
        } = self
        {
            Some(
                statements
                    .iter_mut()
                    .flat_map(
                        IntoTargetsIter::into_trgs_iter
                            as fn(&'a mut S) -> <&'a mut S as IntoTargetsIter>::IntoTrgsIter,
                    )
                    .chain(condjmp.iter_mut().flat_map(
                        IntoTargetsIter::into_trgs_iter
                            as fn(&'a mut C) -> <&'a mut C as IntoTargetsIter>::IntoTrgsIter,
                    ))
                    .chain(next.into_trgs_iter()),
            )
        } else {
            None
        }
        .into_iter()
        .flatten()
    }
}

// simplify trait impls by a bit of cheating...

impl<S, C, T> core::ops::Deref for BasicBlock<S, C, T> {
    type Target = BasicBlockInner<S, C, T>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S, C, T> core::ops::DerefMut for BasicBlock<S, C, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
