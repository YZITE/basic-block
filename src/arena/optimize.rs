use super::{Arena, ABB};
use crate::bb::BasicBlockInner;
use crate::jump::{self, ForeachTarget};
use crate::BbId;
use alloc::collections::{btree_map::Entry as MapEntry, BTreeMap as Map, BTreeSet};
use alloc::vec::Vec;
use core::mem::{drop, take};

impl<S, C> Arena<S, C>
where
    ABB<S, C>: ForeachTarget<JumpTarget = BbId>,
{
    pub fn optimize(&mut self) -> bool {
        let mut max = self.bbs.len();
        let mut trm = Map::new();
        let mut new_in_use = Vec::with_capacity(max);

        let mut mark = |from: BbId, to: Option<BbId>| {
            if max < from {
                max = from;
            }
            if let Some(to2) = to {
                if from == to2 {
                    return;
                }
                if max < to2 {
                    max = to2;
                }
            }
            trm.insert(from, to);
        };

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
                    if let jump::Unconditional::Jump(trg) = *next {
                        mark(n, Some(trg));
                    }
                }
            }
        }

        // recursively mark anything as in-use only if reachable from in-use or pub
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

        let mut offset: BbId = 0;
        for i in 0..self.bbs.len() {
            if !in_use.contains(&i) {
                mark(i, None);
                offset += 1;
            }
        }
        let modified = !trm.is_empty();

        // finalize trm
        for i in 0..max {
            if let &mut Some(x) = trm.entry(i).or_insert(Some(i)) {
                *trm.get_mut(&i).unwrap().as_mut().unwrap() -=
                    trm.iter().take(x).filter(|&(_, j)| j.is_none()).count();
            }
        }

        let trm_get = |n: usize| trm.get(&n).copied();
        let offset = offset;

        // remove to-be-removed BBs
        for (&n, i) in trm.iter().rev() {
            if i.is_none() {
                self.bbs.remove(n);
            }
        }

        // replace jump targets
        self.foreach_target_mut(|target: &mut usize| {
            if let Some(x) = trm_get(*target) {
                if let Some(y) = x {
                    *target = y;
                } else {
                    unreachable!();
                }
            } else {
                // got invalid target
                *target -= offset;
            }
        });

        self.labels = take(&mut self.labels)
            .into_iter()
            .filter(|&(_, bbid)| {
                // remove all labels which point to a BBID which should be deleted,
                // either because it is marked in trm -> None, or the BBID is invalid.
                trm_get(bbid).unwrap_or(None).is_some()
            })
            .collect();

        modified
    }
}
