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
/// To avoid reallocations and multiple intermediate data types,
/// this structure goes through multiple stages, which contain
/// similiar, but not equivalent sets of data.
#[derive(Clone)]
struct TransInfo {
    /// for $key->$target search-and-replace; default = $key
    target: usize,

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
            refs: BTreeSet::new(),
        }
    }
}

/// check function which makes sure that no reference to $exclude exists
/// inside of $container.
fn fetchk<C>(is_mergable: &mut bool, container: &C, exclude: <C as ForeachTarget>::JumpTarget)
where
    C: ForeachTarget,
    C::JumpTarget: Copy + PartialEq,
{
    container.foreach_target(move |&t| {
        if exclude == t {
            *is_mergable = false;
        }
    });
}

impl<S, C> Arena<S, C>
where
    S: ForeachTarget<JumpTarget = BbId>,
    C: ForeachTarget<JumpTarget = BbId>,
{
    pub fn optimize(&mut self) -> bool {
        let mut new_in_use = Vec::with_capacity(self.bbs.len());
        let mut trm: Map<BbId, Option<TransInfo>> = Map::new();

        for (&from, i) in self.bbs.iter() {
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
                        if from != trg {
                            trm.entry(from)
                                .or_insert_with(|| Some(TransInfo::new(from)))
                                .as_mut()
                                .unwrap()
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
                if let Some(ent) = self.bbs.get(&i) {
                    if in_use.insert(i) {
                        // really new entry
                        ent.foreach_target(|&trg| {
                            trm.entry(trg)
                                .or_insert_with(|| Some(TransInfo::new(trg)))
                                .as_mut()
                                .unwrap()
                                .refs
                                .insert(i);
                            new_in_use.push(trg);
                        });
                    }
                }
            }
        }
        drop(new_in_use);

        // check all references for one-refs (which may be mergable)
        for (n, ti) in trm
            .iter()
            .filter_map(|(&n, ti)| ti.as_ref().map(|ti2| (n, ti2)))
        {
            if ti.refs.len() != 1 {
                continue;
            }
            let bbheadref = *ti.refs.iter().next().unwrap();
            if bbheadref == n {
                continue;
            }
            if trm
                .get(&bbheadref)
                .and_then(Option::as_ref)
                .map(|hti| hti.target != bbheadref)
                == Some(true)
            {
                // head is already redirected, skip
                continue;
            }
            let mut is_mergable = false;
            if let Some(bbhead) = self.bbs.get_mut(&bbheadref) {
                if let BasicBlockInner::Concrete {
                    statements,
                    condjmp,
                    next,
                } = &mut bbhead.inner
                {
                    is_mergable = condjmp.is_none() && *next == jump::Unconditional::Jump(n);

                    // make sure that we don't have any additional references to bbtail
                    fetchk(&mut is_mergable, statements, n);
                }
            }
            if !is_mergable {
                continue;
            }
            let bbtail = if let Some(bbtail) = self.bbs.get_mut(&n) {
                if bbtail.is_public || !bbtail.inner.is_concrete() {
                    continue;
                }
                fetchk(&mut is_mergable, bbtail, n);
                if !is_mergable {
                    continue;
                }
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
                } = &mut self.bbs.get_mut(&bbheadref).unwrap().inner
                {
                    in_use.remove(&n);
                    if h_statements.is_empty() {
                        // this normally only happens if $head.is_public
                        // merge labels manually
                        for ltrg in self.labels.values_mut() {
                            if *ltrg == n {
                                *ltrg = bbheadref;
                            }
                        }
                    }
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

        // apply in_use
        let mut modified = false;
        for i in self.bbs.keys() {
            let e = trm.entry(*i);
            if in_use.contains(i) {
                if let alloc::collections::btree_map::Entry::Occupied(mut e) = e {
                    if let Some(ti) = e.get_mut() {
                        if *i != ti.target {
                            modified = true;
                        } else {
                            e.remove_entry();
                        }
                    }
                }
            } else {
                *e.or_default() = None;
                modified = true;
            }
        }
        if !modified {
            // nothing left to do, we are done
            return false;
        }

        // remove to-be-removed BBs
        {
            let mut it = trm.iter().filter(|x| x.1.is_none()).map(|x| x.0);
            if let Some(&nfi) = it.next() {
                if nfi < self.cache_ins_start {
                    self.cache_ins_start = nfi;
                }
                self.bbs.remove(&nfi);
                for n in it {
                    self.bbs.remove(&n);
                }
            }
        }

        // replace jump targets
        for i in self.bbs.values_mut() {
            i.foreach_target_mut(|target| {
                if let Some(Some(ti)) = trm.get(target) {
                    *target = ti.target;
                }
            });
        }

        self.labels = take(&mut self.labels)
            .into_iter()
            .filter_map(|(label, bbid)| {
                // remove all labels which point to a BBID which should be deleted,
                // either because it is marked in trm -> None, or the BBID is invalid.
                // update all remaining labels
                match trm.get(&bbid) {
                    Some(None) => None,
                    Some(Some(ti)) => Some(ti.target),
                    None => Some(bbid),
                }
                .map(|nn| (label, nn))
            })
            .collect();

        true
    }
}
