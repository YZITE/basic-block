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

pub trait IntoTargetsIter {
    type Target;
    type IntoTrgsIter: Iterator<Item = Self::Target>;

    fn into_trgs_iter(self) -> Self::IntoTrgsIter;
}

impl<'a, T> IntoTargetsIter for &'a Dummy<T> {
    type Target = &'a T;
    type IntoTrgsIter = iter::Empty<&'a T>;
    #[inline(always)]
    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        iter::empty()
    }
}

impl<'a, T> IntoTargetsIter for &'a mut Dummy<T> {
    type Target = &'a mut T;
    type IntoTrgsIter = iter::Empty<&'a mut T>;
    #[inline(always)]
    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        iter::empty()
    }
}

impl<'a, T> IntoTargetsIter for &'a Unconditional<T> {
    type Target = &'a T;
    type IntoTrgsIter = core::option::IntoIter<&'a T>;
    #[inline]
    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        if let Unconditional::Jump(t) = self {
            Some(t)
        } else {
            None
        }
        .into_iter()
    }
}

impl<'a, T> IntoTargetsIter for &'a mut Unconditional<T> {
    type Target = &'a mut T;
    type IntoTrgsIter = core::option::IntoIter<&'a mut T>;
    #[inline]
    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        if let Unconditional::Jump(t) = self {
            Some(t)
        } else {
            None
        }
        .into_iter()
    }
}
