use crate::{ArenaJumpTarget as JumpTarget, BbId};
use alloc::{collections::BTreeMap, vec::Vec};
use core::cmp;

pub struct ReplaceLabels {
    trm: BTreeMap<JumpTarget, Option<JumpTarget>>,
    max: JumpTarget,
}

impl ReplaceLabels {
    pub fn new(max: JumpTarget) -> Self {
        Self {
            trm: BTreeMap::new(),
            max,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.trm.is_empty()
    }

    pub fn mark(&mut self, from: JumpTarget, to: Option<JumpTarget>) {
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

    pub fn finalize(mut self) -> (Vec<Option<JumpTarget>>, JumpTarget) {
        let mut offset: JumpTarget = 0;
        let trm2: Vec<Option<JumpTarget>> = {
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
