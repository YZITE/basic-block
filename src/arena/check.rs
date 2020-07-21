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
    fn check_intern(
        &self,
        bbid: BbId,
        bb: &ABB<S, C>,
        allow_new: bool,
        offending: &mut Vec<(BbId, BbId)>,
    ) {
        let endid = self.bbs.len() + if allow_new { 1 } else { 0 };
        bb.foreach_target(|&t| {
            if t >= endid {
                offending.push((bbid, t));
            }
        });
    }

    fn check_bbs(&self) -> Vec<(BbId, BbId)> {
        let mut errs = Vec::new();
        for (n, i) in self.bbs.iter().enumerate() {
            self.check_intern(n, i, false, &mut errs);
        }
        errs
    }

    /// Use this method to re-check all references in the `Arena` after
    /// modifications via [`Arena::bbs_mut`].
    pub fn check(&self) -> Result<(), OffendingIds> {
        let mut errs = self.check_bbs();
        // all labels should point to a valid BbId
        errs.extend(self.labels.iter().filter_map(|(_, &i)| {
            if i >= self.bbs.len() {
                Some((i, i))
            } else {
                None
            }
        }));
        // all placeholders should have label(s)
        for (n, i) in self.bbs.iter().enumerate() {
            if i.inner.is_placeholder() && self.labels_of_bb(n).next().is_none() {
                errs.push((n, n));
            }
        }
        check_finish(errs)
    }

    /// Returns the ID of the newly appended BB if successful,
    /// or $bb & the invalid BbIds.
    pub fn push(&mut self, bb: ABB<S, C>) -> Result<usize, (ABB<S, C>, OffendingIds)> {
        let ret = self.bbs.len();
        let mut errs = Vec::new();
        self.check_intern(ret, &bb, true, &mut errs);
        match check_finish(errs) {
            Ok(()) => {
                self.bbs.push(bb);
                Ok(ret)
            }
            Err(errs) => Err((bb, errs)),
        }
    }

    /// Removes the last BB, fails if any references to it exist.
    /// If successful, returns the removed BB and all labels which referenced it.
    /// Otherwise, returns the offending BBs (which still reference it)
    pub fn pop(&mut self) -> Option<Result<(usize, ABB<S, C>, Vec<String>), OffendingIds>> {
        let x = self.bbs.pop()?;
        let offending = self.check_bbs();
        Some(if offending.is_empty() {
            let retid = self.bbs.len();
            let (labelrt, rlabels) = take(&mut self.labels)
                .into_iter()
                .partition(|&(_, v)| v == retid);
            self.labels = rlabels;
            Ok((retid, x, labelrt.into_iter().map(|x| x.0).collect()))
        } else {
            self.bbs.push(x);
            Err(OffendingIds(offending))
        })
    }
}
