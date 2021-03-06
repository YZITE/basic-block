use crate::bb::{BasicBlock, BasicBlockInner};
use crate::jump::ForeachTarget;
use crate::{BbId, Label};
use alloc::collections::{btree_map::Entry as MapEntry, BTreeMap as Map};
use alloc::{string::String, vec::Vec};
use core::mem::{replace, take};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

mod check;
mod optimize;

type ABB<S, C> = BasicBlock<S, C, BbId>;
type LabelMap = Map<String, BbId>;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Arena<S, C> {
    // invariant: every pointer to another BB should be valid inside the arena.
    bbs: Map<BbId, ABB<S, C>>,
    labels: LabelMap,

    // cache earliest insert point, used to speed up 'push' calls.
    #[cfg_attr(feature = "serde", serde(skip))]
    cache_ins_start: usize,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub enum SetBbLabelError {
    #[cfg_attr(feature = "std", error("got invalid basic block id {0}"))]
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
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct OffendingIds(pub Vec<(BbId, BbId)>);

impl<S, C> Default for Arena<S, C> {
    #[inline]
    fn default() -> Self {
        Self {
            bbs: Map::new(),
            labels: Map::new(),
            cache_ins_start: 0,
        }
    }
}

fn labels_of_bb(labels: &LabelMap, bbid: BbId) -> impl Iterator<Item = &str> {
    labels.iter().filter_map(move |(label, &curid)| {
        if curid == bbid {
            Some(label.as_str())
        } else {
            None
        }
    })
}

fn set_label(
    labels: &mut LabelMap,
    label: String,
    target: BbId,
    overwrite: bool,
) -> Result<Option<BbId>, SetBbLabelError> {
    match labels.entry(label) {
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

impl<S, C> Arena<S, C> {
    #[inline(always)]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.bbs.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.bbs.is_empty()
    }

    #[inline(always)]
    pub fn bbs(&self) -> &Map<BbId, ABB<S, C>> {
        &self.bbs
    }

    #[inline(always)]
    pub fn bbs_mut(&mut self) -> &mut Map<BbId, ABB<S, C>> {
        &mut self.bbs
    }

    #[inline(always)]
    pub fn labels(&self) -> &LabelMap {
        &self.labels
    }

    #[inline(always)]
    pub fn labels_of_bb(&self, bbid: BbId) -> impl Iterator<Item = &str> {
        labels_of_bb(&self.labels, bbid)
    }

    pub fn label2bb(&self, label: &str) -> Option<(BbId, &ABB<S, C>)> {
        if let Some(bbid) = self.labels.get(label) {
            if let Some(bb) = self.bbs.get(bbid) {
                return Some((*bbid, bb));
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
        if self.bbs.get(&target).is_none() {
            return Err(SetBbLabelError::InvalidId(target));
        }
        set_label(&mut self.labels, label.into_owned(), target, overwrite)
    }

    pub fn shrink_to_fit(&mut self) {
        for i in self.bbs.values_mut() {
            if let BasicBlockInner::Concrete { statements, .. } = &mut i.inner {
                statements.shrink_to_fit();
            }
        }
    }
}

impl<S, C> ForeachTarget for Arena<S, C>
where
    ABB<S, C>: ForeachTarget<JumpTarget = BbId>,
{
    type JumpTarget = BbId;

    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        for i in self.bbs.values() {
            i.foreach_target(&mut f);
        }
        for i in self.labels.values() {
            f(i);
        }
    }

    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        for i in self.bbs.values_mut() {
            i.foreach_target_mut(&mut f);
        }
        for i in self.labels.values_mut() {
            f(i);
        }
    }
}
