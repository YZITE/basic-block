use crate::bb::{BasicBlock, BasicBlockInner};
use crate::jump::ForeachTarget;
use crate::{BbId, Label};
use alloc::collections::{btree_map::Entry as MapEntry, BTreeMap as Map};
use alloc::{string::String, vec::Vec};
use core::mem::{replace, take};

mod check;
mod optimize;

type ABB<S, C> = BasicBlock<S, C, BbId>;

#[derive(Debug)]
pub struct Arena<S, C> {
    // invariant: every pointer to another BB should be valid inside the arena.
    bbs: Vec<ABB<S, C>>,
    labels: Map<String, usize>,
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

impl<S, C> ForeachTarget for Arena<S, C>
where
    ABB<S, C>: ForeachTarget<JumpTarget = BbId>,
{
    type JumpTarget = BbId;

    #[inline]
    fn foreach_target<F>(&self, f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        self.bbs.foreach_target(f);
    }

    #[inline]
    fn foreach_target_mut<F>(&mut self, f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        self.bbs.foreach_target_mut(f);
    }
}
