use super::*;

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
    fn check_intern(&self, bbid: BbId, bb: &ABB<S, C>, offending: &mut Vec<(BbId, BbId)>) {
        bb.foreach_target(|&t| {
            if t != bbid && self.bbs.get(&t).is_none() {
                offending.push((bbid, t));
            }
        });
    }

    fn check_bbs(&self) -> Vec<(BbId, BbId)> {
        let mut errs = Vec::new();
        for (&n, i) in self.bbs.iter() {
            self.check_intern(n, i, &mut errs);
        }
        errs
    }

    /// Use this method to re-check all references in the `Arena` after
    /// modifications via [`Arena::bbs_mut`].
    pub fn check(&self) -> Result<(), OffendingIds> {
        let mut errs = self.check_bbs();
        // all labels should point to a valid BbId
        errs.extend(self.labels.values().filter_map(|&i| {
            if self.bbs.get(&i).is_none() {
                Some((i, i))
            } else {
                None
            }
        }));
        // all placeholders should have label(s)
        for (&n, i) in self.bbs.iter() {
            if i.inner.is_placeholder() && self.labels_of_bb(n).next().is_none() {
                errs.push((n, n));
            }
        }
        check_finish(errs)
    }

    fn find_first_free(&self) -> Option<usize> {
        for i in self.cache_ins_start..usize::MAX {
            if self.bbs.get(&i).is_none() {
                return Some(i);
            }
        }
        None
    }

    /// Returns the ID of the newly appended BB if successful,
    /// or $bb & the invalid BbIds.
    pub fn push(&mut self, bb: ABB<S, C>) -> Result<usize, (ABB<S, C>, OffendingIds)> {
        let ret = match self.find_first_free() {
            Some(n) => n,
            None => return Err((bb, OffendingIds(Vec::new()))),
        };
        let mut errs = Vec::new();
        self.check_intern(ret, &bb, &mut errs);
        match check_finish(errs) {
            Ok(()) => {
                self.bbs.insert(ret, bb);
                self.cache_ins_start = ret.saturating_add(1);
                Ok(ret)
            }
            Err(errs) => Err((bb, errs)),
        }
    }

    /// Removes a BB, fails if any references to it exist.
    /// If successful, returns the removed BB and all labels which referenced it.
    /// Otherwise, returns the offending BBs (which still reference it)
    pub fn remove(&mut self, bbid: BbId) -> Option<Result<(ABB<S, C>, Vec<String>), OffendingIds>> {
        let x = self.bbs.remove(&bbid)?;
        let offending = self.check_bbs();
        Some(if offending.is_empty() {
            let (labelrt, rlabels) = take(&mut self.labels)
                .into_iter()
                .partition(|&(_, v)| v == bbid);
            self.labels = rlabels;
            if bbid < self.cache_ins_start {
                self.cache_ins_start = bbid;
            }
            Ok((x, labelrt.into_iter().map(|x| x.0).collect()))
        } else {
            self.bbs.insert(bbid, x);
            Err(OffendingIds(offending))
        })
    }
}
