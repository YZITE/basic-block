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
    assert_eq!(arena.len(), 0);
    arena.check().unwrap()
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
    assert_eq!(arena.len(), 1);
    arena.check().unwrap();
    arena.optimize();
    assert_eq!(arena.len(), 1);
    arena.bbs.get_mut(&0).unwrap().is_public = false;
    arena.check().unwrap();
    arena.optimize();
    assert_eq!(arena.len(), 0);
    arena.check().unwrap();
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
    let id = pr.unwrap();
    assert!(arena.set_label("main".into(), id, false).is_ok());
    if let Some(Ok(_)) = arena.remove(id) {
        // do nothing
    } else {
        unreachable!();
    }
}

#[derive(Debug)]
struct LolCondJmp<T> {
    target: T,
}

impl<'a, T> yz_basic_block::jump::IntoTargetsIter for &'a LolCondJmp<T> {
    type Target = &'a T;
    type IntoTrgsIter = core::iter::Once<&'a T>;
    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        core::iter::once(&self.target)
    }
}

impl<'a, T> yz_basic_block::jump::IntoTargetsIter for &'a mut LolCondJmp<T> {
    type Target = &'a mut T;
    type IntoTrgsIter = core::iter::Once<&'a mut T>;
    fn into_trgs_iter(self) -> Self::IntoTrgsIter {
        core::iter::once(&mut self.target)
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

    if let BasicBlockInner::Concrete { condjmp, .. } = &mut arena.bbs.get_mut(&0).unwrap().inner {
        // link both BBs together
        *condjmp = Some(LolCondJmp { target: 1 });
    }
    assert!(arena.check().is_ok());

    // .pop should fail
    assert_eq!(arena.remove(1).unwrap().unwrap_err().0, &[(0, 1)]);

    // optimize should keep it
    arena.optimize();
    eprintln!("{:?}", arena);
    assert_eq!(arena.len(), 2);

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

    if let BasicBlockInner::Concrete { next, .. } = &mut arena.bbs.get_mut(&0).unwrap().inner {
        // link both BBs together
        *next = Unconditional::Jump(1);
    }
    assert!(arena.check().is_ok());

    // .pop should fail
    assert_eq!(arena.remove(1).unwrap().unwrap_err().0, &[(0, 1)]);

    // optimize should merge it
    arena.optimize();
    assert_eq!(arena.len(), 1);
    assert!(arena.check().is_ok());

    if let BasicBlockInner::Concrete { next, .. } = &arena.bbs[&0].inner {
        assert_eq!(*next, Unconditional::Return);
    } else {
        unreachable!();
    }
}
