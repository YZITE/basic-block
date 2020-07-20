#[allow(unused_imports)]
use yz_basic_block::{
    jump::{Dummy, Unconditional},
    Arena, BasicBlock,
};

type DummyArena = Arena<Dummy<usize>, ()>;

#[test]
fn bb0() {
    let mut arena = DummyArena::new();
    arena.optimize();
    assert_eq!(arena.bbs().len(), 0);
}

#[test]
fn bb1() {
    let mut arena = DummyArena::new();
    let pr = arena.push(BasicBlock {
        statements: Vec::new(),
        condjmp: None,
        next: Unconditional::Halt,
        is_public: true,
    });
    assert!(pr.is_ok());
    assert!(arena.set_label("main".into(), pr.unwrap(), false).is_ok());
    assert!(arena.set_label("main".into(), 0, false).is_err());
    assert!(arena.set_label("main".into(), 1, true).is_err());
    let pr = arena.push(BasicBlock {
        statements: Vec::new(),
        condjmp: None,
        next: Unconditional::Jump(5),
        is_public: true,
    });
    assert!(pr.is_err());
    assert_eq!(arena.bbs().len(), 1);
    arena.optimize();
    assert_eq!(arena.bbs().len(), 1);
    arena.bbs_mut()[0].is_public = false;
    arena.optimize();
    assert_eq!(arena.bbs().len(), 0);
}
