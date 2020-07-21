use super::Arena;
use crate::bb::BasicBlockInner;
use crate::jump::{self, ForeachTarget};
use crate::BbId;
use alloc::collections::{BTreeMap as Map, BTreeSet};
use alloc::vec::Vec;
use core::mem::{drop, take};

/// Temporary information describing the modifications to be done
/// and cached data, per BbId.
///
/// This structure is at least implicitly wrapped in an `Option` (key exists?),
/// if Some, then TR $key->$target,
/// otherwise, remove every occurence of $key if possible,
/// panic otherwise (especially if otherwise invariants would be violated)
///
/// after applying $key->$target, do
/// ```do_ignore
///   let mut t = &mut $trm[$i].target;
///   *t -= $trm[*t].offset;
/// ```
///
/// To avoid reallocations and multiple intermediate data types,
/// this structure goes through multiple stages, which contain
/// similiar, but not equivalent sets of data.
/// $offset is calculated at a different stage than $target,
/// and shouldn't be trusted earlier.
#[derive(Clone)]
struct TransInfo {
    /// for $key->$target search-and-replace; default = $key
    target: usize,

    /// offset correction for $key ($target -= $target.$offset) to
    /// accomodate removed BBs
    offset: Option<usize>,

    /// references from $i pointing to $key,
    /// useful for optimizations which rely on the fact that only
    /// one other BB depends on a BB, then they're mergable
    /// default = 1
    refs: BTreeSet<usize>,
}

impl TransInfo {
    #[inline]
    fn new(id: usize) -> Self {
        Self {
            target: id,
            offset: None,
            refs: BTreeSet::new(),
        }
    }
}

impl<S, C> Arena<S, C>
where
    S: ForeachTarget<JumpTarget = BbId>,
    C: ForeachTarget<JumpTarget = BbId>,
{
    pub fn optimize(&mut self) -> bool {
        let mut max = self.bbs.len();
        let mut new_in_use = Vec::with_capacity(max);

        let mut trm: Map<BbId, TransInfo> = (0..max).map(|i| (i, TransInfo::new(i))).collect();

        for (from, i) in self.bbs.iter().enumerate() {
            if i.is_public {
                new_in_use.push(from);
            } else if let BasicBlockInner::Concrete {
                statements,
                condjmp,
                next,
            } = &i.inner
            {
                if statements.is_empty() && condjmp.is_none() {
                    if let jump::Unconditional::Jump(trg) = *next {
                        let tmp_max = core::cmp::max(from, trg);
                        if max < tmp_max {
                            max = tmp_max;
                        }
                        if from != trg {
                            trm.entry(from)
                                .or_insert_with(|| TransInfo::new(from))
                                .target = trg;
                        }
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
                    self.bbs[i].foreach_target(|&trg| {
                        trm.entry(trg)
                            .or_insert_with(|| TransInfo::new(trg))
                            .refs
                            .insert(i);
                        new_in_use.push(trg);
                    });
                }
            }
        }
        drop(new_in_use);

        // check all references for one-refs (which may be mergable)
        for (&n, ti) in trm.iter() {
            if ti.refs.len() != 1 {
                continue;
            }
            let bbheadref = *ti.refs.iter().next().unwrap();
            if bbheadref == n {
                continue;
            }
            let mut is_mergable = false;
            if let Some(bbhead) = self.bbs.get_mut(bbheadref) {
                if let BasicBlockInner::Concrete {
                    statements,
                    condjmp,
                    next,
                } = &mut bbhead.inner
                {
                    is_mergable = condjmp.is_none() && *next == jump::Unconditional::Jump(n);
                    // make sure that we don't have any additional references to bbtail
                    (*statements).foreach_target(|&t| {
                        if t == n {
                            is_mergable = false;
                        }
                    });
                }
            }
            if !is_mergable {
                continue;
            }
            let bbtail = if let Some(bbtail) = self.bbs.get_mut(n) {
                if bbtail.is_public || !bbtail.inner.is_concrete() {
                    continue;
                }
                bbtail.foreach_target(|&t| assert_ne!(n, t));
                take(&mut bbtail.inner)
            } else {
                continue;
            };

            // mergable
            if let BasicBlockInner::Concrete {
                mut statements,
                condjmp,
                next,
            } = bbtail
            {
                if let BasicBlockInner::Concrete {
                    statements: ref mut h_statements,
                    condjmp: ref mut h_condjmp,
                    next: ref mut h_next,
                } = &mut self.bbs.get_mut(bbheadref).unwrap().inner
                {
                    in_use.remove(&n);
                    h_statements.append(&mut statements);
                    *h_condjmp = condjmp;
                    *h_next = next;
                } else {
                    unreachable!();
                }
            } else {
                unreachable!();
            }
        }

        // calculate offsets, apply in_use
        let mut offset: BbId = 0;
        let mut modified = false;
        assert!(max >= self.bbs.len());
        for i in 0..self.bbs.len() {
            if in_use.contains(&i) {
                let e = trm.get_mut(&i).unwrap();
                if i != e.target {
                    modified = true;
                }
                e.offset = Some(offset);
            } else {
                trm.remove(&i);
                offset += 1;
            }
        }
        if offset == 0 && !modified {
            // nothing left to do, we are done
            return false;
        }
        // avoid repetition + re-execution of `unwrap_or(TransInfo::new(...))...`
        for i in self.bbs.len()..max {
            trm.entry(i).or_insert_with(|| TransInfo::new(i)).offset = Some(offset);
        }

        // finalize trm, apply offset correction to each $target
        for i in 0..max {
            if let Some(ti) = trm.get_mut(&i) {
                if i == ti.target {
                    ti.target -= ti.offset.unwrap();
                } else {
                    let old_target = ti.target;
                    let new_target = old_target - trm.get(&old_target).unwrap().offset.unwrap();
                    trm.get_mut(&i).unwrap().target = new_target;
                }
            }
        }

        let trm_get = |n: usize| trm.get(&n).map(|ti| ti.target);

        // remove to-be-removed BBs
        for n in (0..max).rev() {
            if trm.get(&n).is_none() {
                self.bbs.remove(n);
            }
        }

        // replace jump targets
        self.bbs.foreach_target_mut(|target: &mut usize| {
            *target = trm_get(*target).expect("violated invariant");
        });

        self.labels = take(&mut self.labels)
            .into_iter()
            .filter(|(_, bbid)| {
                // remove all labels which point to a BBID which should be deleted,
                // either because it is marked in trm -> None, or the BBID is invalid.
                trm.get(bbid).is_some()
            })
            .collect();

        true
    }
}
