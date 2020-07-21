#[allow(unused_imports)]
use yz_basic_block::{
    jump::{Dummy, Unconditional},
    Arena, BasicBlock, BasicBlockInner,
};

type DummyArena = Arena<Dummy<usize>, Dummy<usize>>;

#[test]
fn bb0() {
    let mut arena = DummyArena::new();
    assert!(arena.check().is_ok());
    arena.optimize();
    assert_eq!(arena.bbs().len(), 0);
    assert!(arena.check().is_ok());
}

#[test]
fn bb1() {
    let mut arena = DummyArena::new();
    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Halt,
        },
        is_public: true,
    });
    assert!(pr.is_ok());
    assert!(arena.set_label("main".into(), pr.unwrap(), false).is_ok());
    assert!(arena.set_label("main".into(), 0, false).is_err());
    assert!(arena.set_label("main".into(), 1, true).is_err());
    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Jump(5),
        },
        is_public: true,
    });
    assert!(pr.is_err());
    assert_eq!(arena.bbs().len(), 1);
    assert!(arena.check().is_ok());
    arena.optimize();
    assert_eq!(arena.bbs().len(), 1);
    arena.bbs_mut()[0].is_public = false;
    assert!(arena.check().is_ok());
    arena.optimize();
    assert_eq!(arena.bbs().len(), 0);
    assert!(arena.check().is_ok());
}

#[test]
fn bb_pop() {
    let mut arena = DummyArena::new();
    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Halt,
        },
        is_public: true,
    });
    assert!(pr.is_ok());
    assert!(arena.set_label("main".into(), pr.unwrap(), false).is_ok());
    if let Some(Ok(_)) = arena.pop() {
        // do nothing
    } else {
        unreachable!();
    }
}

#[derive(Debug)]
struct LolCondJmp<T> {
    target: T,
}

impl<T> yz_basic_block::jump::ForeachTarget for LolCondJmp<T> {
    type JumpTarget = T;

    fn foreach_target<F>(&self, mut f: F)
    where
        F: FnMut(&Self::JumpTarget),
    {
        f(&self.target);
    }

    fn foreach_target_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self::JumpTarget),
    {
        f(&mut self.target);
    }
}

#[test]
fn bb_keep() {
    let mut arena = Arena::<Dummy<usize>, LolCondJmp<usize>>::new();

    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Halt,
        },
        is_public: true,
    });
    assert!(pr.is_ok());
    assert!(arena.set_label("main".into(), pr.unwrap(), false).is_ok());

    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Halt,
        },
        is_public: false,
    });
    assert!(pr.is_ok());

    if let BasicBlockInner::Concrete { condjmp, .. } = &mut arena.bbs_mut()[0].inner {
        // link both BBs together
        *condjmp = Some(LolCondJmp { target: 1 });
    }
    assert!(arena.check().is_ok());

    // .pop should fail
    assert_eq!(arena.pop().unwrap().unwrap_err().0, &[(0, 1)]);

    // optimize should keep it
    arena.optimize();
    eprintln!("{:?}", arena);
    assert_eq!(arena.bbs().len(), 2);

    assert!(arena.check().is_ok());
}

#[test]
fn bb_merge() {
    let mut arena = DummyArena::new();

    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Halt,
        },
        is_public: true,
    });
    assert!(pr.is_ok());
    assert!(arena.set_label("main".into(), pr.unwrap(), false).is_ok());

    let pr = arena.push(BasicBlock {
        inner: BasicBlockInner::Concrete {
            statements: Vec::new(),
            condjmp: None,
            next: Unconditional::Return,
        },
        is_public: false,
    });
    assert!(pr.is_ok());

    if let BasicBlockInner::Concrete { next, .. } = &mut arena.bbs_mut()[0].inner {
        // link both BBs together
        *next = Unconditional::Jump(1);
    }
    assert!(arena.check().is_ok());

    // .pop should fail
    assert_eq!(arena.pop().unwrap().unwrap_err().0, &[(0, 1)]);

    // optimize should merge it
    arena.optimize();
    assert_eq!(arena.bbs().len(), 1);
    assert!(arena.check().is_ok());

    if let BasicBlockInner::Concrete { next, .. } = &arena.bbs()[0].inner {
        assert_eq!(*next, Unconditional::Return);
    } else {
        unreachable!();
    }
}
