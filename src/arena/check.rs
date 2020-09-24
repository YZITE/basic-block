use super::*;
use either::Either;

fn check_finish(mut offending: Vec<(BbId, BbId)>) -> Result<(), OffendingIds> {
    if offending.is_empty() {
        Ok(())
    } else {
        offending.sort_unstable();
        offending.dedup();
        offending.shrink_to_fit();
        Err(OffendingIds(offending))
    }
}

impl<S, C> Arena<S, C>
where
    for<'a> &'a BasicBlockInner<S, C, BbId>: IntoTargetsIter<Target = &'a BbId>,
{
    fn check_intern<'a>(
        &'a self,
        bbid: BbId,
        bb: &'a ABB<S, C>,
    ) -> impl Iterator<Item = (BbId, BbId)> + 'a {
        bb.into_trgs_iter()
            .copied()
            .filter(move |t| *t != bbid && self.bbs.get(t).is_none())
            .map(move |t| (bbid, t))
    }

    /// Use this method to re-check all references in the `Arena` after
    /// modifications via [`Arena::bbs`].
    pub fn check(&self) -> Result<(), OffendingIds> {
        let errs: Vec<_> = self
            .bbs
            .iter()
            .flat_map(|(&n, i)| {
                if i.inner.is_placeholder() {
                    // all placeholders should have label(s)
                    Either::Left(
                        if self.labels_of_bb(n).next().is_none() {
                            Some((n, n))
                        } else {
                            None
                        }
                        .into_iter(),
                    )
                } else {
                    Either::Right(self.check_intern(n, i))
                }
            })
            .chain(
                // all labels should point to a valid BbId
                self.labels
                    .values()
                    .filter(|i| self.bbs.get(i).is_none())
                    .map(|&i| (i, i)),
            )
            .collect();
        check_finish(errs)
    }

    fn find_first_free(&self) -> Option<usize> {
        (self.cache_ins_start..usize::MAX).find(|i| self.bbs.get(i).is_none())
    }

    /// Returns the ID of the newly appended BB if successful,
    /// or $bb & the invalid BbIds.
    pub fn push(&mut self, bb: ABB<S, C>) -> Result<usize, (ABB<S, C>, OffendingIds)> {
        let ret = match self.find_first_free() {
            Some(n) => n,
            None => return Err((bb, OffendingIds(Vec::new()))),
        };
        let errs: Vec<_> = self.check_intern(ret, &bb).collect();
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
        let offending: Vec<_> = self
            .bbs
            .iter()
            .flat_map(|(&n, i)| self.check_intern(n, i))
            .collect();
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
