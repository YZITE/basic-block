use crate::ArenaJumpTarget as JumpTarget;
use std::cmp;
use std::collections::BTreeMap;

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
