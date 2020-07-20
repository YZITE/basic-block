#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;
extern crate core;

use alloc::collections::{btree_map::Entry as MapEntry, BTreeMap as Map, BTreeSet};
use alloc::{string::String, vec::Vec};
use core::mem::{drop, replace, take};

mod helpers;
pub mod jump;

pub use helpers::{OffendingIds, SetBbLabelError};
use jump::ForeachTarget;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BasicBlockInner<S, C, T> {
    Concrete {
        statements: Vec<S>,
        condjmp: Option<jump::Conditional<C, T>>,
        next: jump::Unconditional<T>,
    },
    /// placeholder for linker references to other files
    Placeholder { is_extern: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasicBlock<S, C, T> {
    pub inner: BasicBlockInner<S, C, T>,
    pub is_public: bool,
}

impl<S, C, T> ForeachTarget for BasicBlockInner<S, C, T>
where
    S: ForeachTarget<JumpTarget = T>,
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
{
    type JumpTarget = T;

    #[inline]
    fn foreach_target<F>(&self, f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        self.inner.foreach_target(f);
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        self.inner.foreach_target_mut(f);
    }
}

pub type BbId = usize;
pub type Label = alloc::borrow::Cow<'static, str>;
type ArenaJumpTarget = BbId;
type ABB<S, C> = BasicBlock<S, C, ArenaJumpTarget>;

pub struct Arena<S, C> {
    // invariant: every pointer to another BB should be valid inside the arena.
    bbs: Vec<ABB<S, C>>,
    labels: Map<String, usize>,
}

impl<S, C> Default for Arena<S, C> {
    #[inline]
    fn default() -> Self {
        Self {
            bbs: Vec::new(),
            labels: Map::new(),
        }
    }
}

impl<S, C> Arena<S, C> {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn bbs(&self) -> &[ABB<S, C>] {
        &self.bbs[..]
    }

    #[inline]
    pub fn bbs_mut(&mut self) -> &mut [ABB<S, C>] {
        &mut self.bbs[..]
    }

    pub fn labels_of_bb(&self, bbid: BbId) -> impl Iterator<Item = &str> {
        self.labels.iter().flat_map(move |(label, &curid)| {
            if curid == bbid {
                Some(label.as_str())
            } else {
                None
            }
        })
    }

    pub fn label2bb(&self, label: &str) -> Option<(BbId, &ABB<S, C>)> {
        if let Some(&bbid) = self.labels.get(label) {
            if let Some(bb) = self.bbs.get(bbid) {
                return Some((bbid, bb));
            }
        }
        None
    }

    /// If this call replaced the current label->BB-ID association,
    /// then the old associated BBID is returned.
    pub fn set_label(
        &mut self,
        label: Label,
        target: BbId,
        overwrite: bool,
    ) -> Result<Option<BbId>, SetBbLabelError> {
        if target >= self.bbs.len() {
            return Err(SetBbLabelError::InvalidId(target));
        }
        match self.labels.entry(label.into_owned()) {
            MapEntry::Occupied(mut e) if overwrite => Ok(Some(replace(e.get_mut(), target))),
            MapEntry::Occupied(e) => Err(SetBbLabelError::LabelAlreadyExists {
                orig_target: *e.get(),
            }),
            MapEntry::Vacant(e) => {
                e.insert(target);
                Ok(None)
            }
        }
    }

    pub fn shrink_to_fit(&mut self) {
        for i in &mut self.bbs {
            if let BasicBlockInner::Concrete { statements, .. } = &mut i.inner {
                statements.shrink_to_fit();
            }
        }
        self.bbs.shrink_to_fit();
    }
}

fn check_finish(mut offending: Vec<BbId>) -> Result<(), OffendingIds> {
    if offending.is_empty() {
        Ok(())
    } else {
        offending.sort();
        offending.dedup();
        offending.shrink_to_fit();
        Err(OffendingIds(offending))
    }
}

impl<S, C> Arena<S, C>
where
    S: ForeachTarget<JumpTarget = ArenaJumpTarget>,
{
    fn check_intern(&self, bb: &ABB<S, C>, allow_new: bool, offending: &mut Vec<BbId>) {
        let endid = self.bbs.len() + if allow_new { 1 } else { 0 };
        bb.foreach_target(|&t| {
            if t >= endid {
                offending.push(t);
            }
        });
    }

    /// Use this method to re-check all references in the `Arena` after
    /// modifications via [`Arena::bbs_mut`].
    pub fn check(&self) -> Result<(), OffendingIds> {
        let mut errs = Vec::new();
        for i in &self.bbs {
            self.check_intern(i, false, &mut errs);
        }
        errs.extend(self.labels.iter().filter_map(|(_, &i)| {
            if i >= self.bbs.len() {
                Some(i)
            } else {
                None
            }
        }));
        check_finish(errs)
    }

    /// Returns the ID of the newly appended BB if successful,
    /// or $bb & the invalid BbIds.
    pub fn push(&mut self, bb: ABB<S, C>) -> Result<usize, (ABB<S, C>, OffendingIds)> {
        let mut errs = Vec::new();
        self.check_intern(&bb, true, &mut errs);
        match check_finish(errs) {
            Ok(()) => {
                let ret = self.bbs.len();
                self.bbs.push(bb);
                Ok(ret)
            }
            Err(errs) => Err((bb, errs)),
        }
    }

    /// Removes the last BB, fails if any references to it exist.
    /// If successful, returns the removed BB and all labels which referenced it.
    /// Otherwise, returns the offending BBs (which still reference it)
    pub fn pop(&mut self) -> Option<Result<(usize, ABB<S, C>, Vec<String>), OffendingIds>> {
        let x = self.bbs.pop()?;
        let retid = self.bbs.len();
        let offending: Vec<BbId> = self
            .bbs
            .iter()
            .enumerate()
            .filter(|(_, bb)| {
                let mut is_offending = false;
                bb.foreach_target(|&t| {
                    if t == retid {
                        is_offending = true;
                    }
                });
                is_offending
            })
            .map(|i| i.0)
            .collect();
        Some(if offending.is_empty() {
            let (labelrt, rlabels) = take(&mut self.labels)
                .into_iter()
                .partition(|&(_, v)| v == retid);
            self.labels = rlabels;
            Ok((retid, x, labelrt.into_iter().map(|x| x.0).collect()))
        } else {
            self.bbs.push(x);
            Err(OffendingIds(offending))
        })
    }

    pub fn optimize(&mut self) -> bool {
        let mut ltr = helpers::ReplaceLabels::new(self.bbs.len());
        let mut new_in_use = Vec::with_capacity(self.bbs.len());
        for (n, i) in self.bbs.iter().enumerate() {
            if i.is_public {
                new_in_use.push(n);
            } else if let BasicBlockInner::Concrete {
                statements,
                condjmp,
                next,
            } = &i.inner
            {
                if statements.is_empty() && condjmp.is_none() {
                    if let jump::Unconditional::Jump(trg) = next {
                        ltr.mark(n, Some(*trg));
                    }
                }
            }
        }

        // recursively mark anything as in-use only if unreachable from in-use or pub
        let mut in_use = BTreeSet::new();
        while !new_in_use.is_empty() {
            for i in take(&mut new_in_use) {
                if in_use.insert(i) {
                    // really new entry
                    self.bbs[i].foreach_target(|&trg| new_in_use.push(trg));
                }
            }
        }
        drop(new_in_use);

        for i in 0..self.bbs.len() {
            if !in_use.contains(&i) {
                ltr.mark(i, None);
            }
        }
        let modified = !ltr.is_empty();
        self.replace_labels(ltr);

        modified
    }

    fn replace_labels(&mut self, labels: helpers::ReplaceLabels) -> bool {
        let (trm, offset) = labels.finalize();
        let mut success = true;

        self.foreach_target_mut(|target: &mut usize| {
            if let Some(x) = trm.get(*target) {
                if let Some(y) = x {
                    *target = *y;
                }
            } else {
                // got invalid target
                *target -= offset;
                success = false;
            }
        });

        for (n, _) in trm.iter().enumerate().rev().filter(|(_, i)| i.is_none()) {
            self.bbs.remove(n);
        }

        self.labels = take(&mut self.labels)
            .into_iter()
            .filter(|(_, bbid)| {
                // remove all labels which point to a BBID which should be deleted,
                // either because it is marked in trm -> None, or the BBID is invalid.
                trm.get(*bbid).unwrap_or(&None).is_some()
            })
            .collect();

        success
    }
}

impl<S, C> ForeachTarget for Arena<S, C>
where
    ABB<S, C>: ForeachTarget<JumpTarget = ArenaJumpTarget>,
{
    type JumpTarget = ArenaJumpTarget;

    #[inline]
    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        for i in &self.bbs {
            i.foreach_target(&mut f);
        }
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        for i in &mut self.bbs {
            i.foreach_target_mut(&mut f);
        }
    }
}
