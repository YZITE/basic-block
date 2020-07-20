use crate::bb::{BasicBlock, BasicBlockInner};
use crate::jump::{self, ForeachTarget};
use crate::{BbId, Label};
use alloc::collections::{btree_map::Entry as MapEntry, BTreeMap as Map, BTreeSet};
use alloc::{string::String, vec::Vec};
use core::cmp;
use core::mem::{drop, replace, take};

type ABB<S, C> = BasicBlock<S, C, BbId>;

pub struct Arena<S, C> {
    // invariant: every pointer to another BB should be valid inside the arena.
    bbs: Vec<ABB<S, C>>,
    labels: Map<String, usize>,
}

pub struct ReplaceLabels {
    trm: Map<BbId, Option<BbId>>,
    max: BbId,
}

impl ReplaceLabels {
    pub fn new(max: BbId) -> Self {
        Self {
            trm: Map::new(),
            max,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.trm.is_empty()
    }

    pub fn mark(&mut self, from: BbId, to: Option<BbId>) {
        self.max = cmp::max(self.max, from);
        if let Some(x) = to {
            self.max = cmp::max(self.max, x);
        }
        if let Some(to2) = to {
            if from == to2 {
                return;
            }
        }
        self.trm.insert(from, to);
    }

    pub fn finalize(mut self) -> (Vec<Option<BbId>>, BbId) {
        let mut offset: BbId = 0;
        let trm2: Vec<Option<BbId>> = {
            (0..self.max)
                .map(|i| {
                    if self.trm.get(&i) == Some(&None) {
                        offset += 1;
                        None
                    } else {
                        Some(i - offset)
                    }
                })
                .collect()
        };
        for i in 0..self.max {
            if let Some(x) = self.trm.get_mut(&i) {
                if let Some(y) = x {
                    *y = trm2[*y].unwrap();
                }
            } else {
                self.trm.insert(i, Some(trm2[i].unwrap()));
            }
        }
        (self.trm.into_iter().map(|(_, i)| i).collect(), offset)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum SetBbLabelError {
    #[cfg_attr(
        feature = "std",
        error("got invalid basic block id {0} (out of range)")
    )]
    InvalidId(BbId),

    #[cfg_attr(
        feature = "std",
        error("label already exists with target = {orig_target}")
    )]
    LabelAlreadyExists { orig_target: BbId },
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[cfg_attr(
    feature = "std",
    error("got offending basic block ids (from -> to) {0:?}")
)]
pub struct OffendingIds(pub Vec<(BbId, BbId)>);

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
    #[inline(always)]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline(always)]
    pub fn bbs(&self) -> &[ABB<S, C>] {
        &self.bbs[..]
    }

    #[inline(always)]
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

fn check_finish(mut offending: Vec<(BbId, BbId)>) -> Result<(), OffendingIds> {
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
    ABB<S, C>: ForeachTarget<JumpTarget = BbId>,
{
    fn check_intern(
        &self,
        bbid: BbId,
        bb: &ABB<S, C>,
        allow_new: bool,
        offending: &mut Vec<(BbId, BbId)>,
    ) {
        let endid = self.bbs.len() + if allow_new { 1 } else { 0 };
        bb.foreach_target(|&t| {
            if t >= endid {
                offending.push((bbid, t));
            }
        });
    }

    fn check_bbs(&self) -> Vec<(BbId, BbId)> {
        let mut errs = Vec::new();
        for (n, i) in self.bbs.iter().enumerate() {
            self.check_intern(n, i, false, &mut errs);
        }
        errs
    }

    /// Use this method to re-check all references in the `Arena` after
    /// modifications via [`Arena::bbs_mut`].
    pub fn check(&self) -> Result<(), OffendingIds> {
        let mut errs = self.check_bbs();
        // all labels should point to a valid BbId
        errs.extend(self.labels.iter().filter_map(|(_, &i)| {
            if i >= self.bbs.len() {
                Some((i, i))
            } else {
                None
            }
        }));
        // all placeholders should have label(s)
        for (n, i) in self.bbs.iter().enumerate() {
            if let BasicBlockInner::Placeholder { .. } = &i.inner {
                if self.labels_of_bb(n).next().is_none() {
                    errs.push((n, n));
                }
            }
        }
        check_finish(errs)
    }

    /// Returns the ID of the newly appended BB if successful,
    /// or $bb & the invalid BbIds.
    pub fn push(&mut self, bb: ABB<S, C>) -> Result<usize, (ABB<S, C>, OffendingIds)> {
        let ret = self.bbs.len();
        let mut errs = Vec::new();
        self.check_intern(ret, &bb, true, &mut errs);
        match check_finish(errs) {
            Ok(()) => {
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
        let offending = self.check_bbs();
        Some(if offending.is_empty() {
            let retid = self.bbs.len();
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
        let mut ltr = ReplaceLabels::new(self.bbs.len());
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

    fn replace_labels(&mut self, labels: ReplaceLabels) -> bool {
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
    ABB<S, C>: ForeachTarget<JumpTarget = BbId>,
{
    type JumpTarget = BbId;

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
