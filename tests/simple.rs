#[allow(unused_imports)]
use yz_basic_block::{Arena, BasicBlock, jump::Dummy};

type DummyArena = Arena<Dummy<usize>, ()>;

#[test]
fn bb0() {
    let mut arena = DummyArena::new();
    arena.optimize();
    assert_eq!(arena.bbs(), &[]);
}
