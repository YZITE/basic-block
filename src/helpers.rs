use crate::ArenaJumpTarget as JumpTarget;
use std::{cmp, collections::BTreeMap};

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

#[derive(Clone, Debug, thiserror::Error)]
pub enum SetBbLabelError {
    #[error("got invalid basic block id {0} (out of range)")]
    InvalidId(crate::BbId),

    #[error("label already exists with target = {orig_target}")]
    LabelAlreadyExists { orig_target: crate::BbId },
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("got offending basic block ids {0:?}")]
pub struct OffendingIds(pub Vec<crate::BbId>);
