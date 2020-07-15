use std::collections::{BTreeSet, HashMap};
use std::mem::{drop, take};

mod helpers;
pub mod jump;

pub struct BasicBlock<S, C> {
    pub labels: Vec<String>,
    pub statements: Vec<S>,
    pub condjmp: Option<jump::Conditional<C>>,
    pub next: jump::Unconditional,
    pub is_public: bool,
}

impl<S, C> BasicBlock<S, C> {
    pub fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(jump::Target),
    {
        if let Some(x) = &self.condjmp {
            f(x.target);
        }
        if let Some(x) = self.next.target() {
            f(x);
        }
    }

    pub fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut jump::Target),
    {
        if let Some(x) = &mut self.condjmp {
            f(&mut x.target);
        }
        if let Some(x) = self.next.target_mut() {
            f(x);
        }
    }
}

pub struct Arena<S, C> {
    // invariant: every pointer to another BB should be valid inside the arena.
    bbs: Vec<BasicBlock<S, C>>,
    labels: HashMap<String, jump::Target>,
}

impl<S, C> Default for Arena<S, C> {
    #[inline]
    fn default() -> Self {
        Self {
            bbs: Vec::new(),
            labels: HashMap::new(),
        }
    }
}

impl<S, C> Arena<S, C> {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn bbs(&self) -> &[BasicBlock<S, C>] {
        &self.bbs[..]
    }

    #[inline]
    pub fn bbs_mut(&mut self) -> &mut [BasicBlock<S, C>] {
        &mut self.bbs[..]
    }

    pub fn labels_of_bb(&self, bbid: jump::Target) -> impl Iterator<Item = &str> {
        self.labels.iter().flat_map(move |(label, &curid)| {
            if curid == bbid {
                Some(label.as_str())
            } else {
                None
            }
        })
    }

    pub fn label2bb(&self, label: &str) -> Option<(jump::Target, &BasicBlock<S, C>)> {
        if let Some(&bbid) = self.labels.get(label) {
            if let Some(bb) = self.bbs.get(bbid) {
                return Some((bbid, bb));
            }
        }
        None
    }

    pub fn shrink_to_fit(&mut self) {
        for i in &mut self.bbs {
            i.labels.shrink_to_fit();
            i.statements.shrink_to_fit();
        }
        self.bbs.shrink_to_fit();
    }

    pub fn optimize(&mut self) -> bool {
        let mut ltr = helpers::ReplaceLabels::new(self.bbs.len());
        let mut in_public = BTreeSet::<jump::Target>::new();
        for (n, i) in self.bbs.iter().enumerate() {
            if i.is_public {
                in_public.insert(n);
            } else if i.statements.is_empty() && i.condjmp.is_none() {
                if let jump::Unconditional::Jump(trg) = i.next {
                    ltr.mark(n, Some(trg));
                }
            }
        }

        // recursively mark anything as in-use only if unreachable from in-use or pub
        let mut in_use = BTreeSet::new();
        let mut new_in_use = in_public;
        while !new_in_use.is_empty() {
            let mut old_in_use = take(&mut new_in_use);

            for &i in old_in_use.iter() {
                self.bbs[i].foreach_target(|trg| {
                    new_in_use.insert(trg);
                });
            }

            in_use.append(&mut old_in_use);
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
        let mut handle_target = |target: &mut usize| {
            if let Some(x) = trm.get(*target) {
                if let Some(y) = x {
                    *target = *y;
                }
            } else {
                // got invalid target
                *target -= offset;
                success = false;
            }
        };

        for i in &mut self.bbs {
            i.foreach_target_mut(&mut handle_target);
        }

        drop(handle_target);

        for (n, _) in trm.iter().enumerate().rev().filter(|(_, i)| i.is_none()) {
            self.bbs.remove(n);
        }

        self.labels.retain(|_, v| {
            // remove all labels which either
            if let Some(x) = trm.get(*v) {
                if let Some(y) = x {
                    *v = *y;
                }
                x.is_some()
            } else {
                false
            }
        });

        success
    }
}
