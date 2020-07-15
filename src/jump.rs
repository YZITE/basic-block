pub type Target = usize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conditional<C> {
    pub target: Target,
    pub condition: C,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unconditional {
    Halt,
    Jump(Target),
    Return,
    Unknown,
}

impl Unconditional {
    pub fn target(&self) -> Option<Target> {
        if let Unconditional::Jump(t) = self {
            Some(*t)
        } else {
            None
        }
    }

    pub fn target_mut(&mut self) -> Option<&mut Target> {
        if let Unconditional::Jump(t) = self {
            Some(t)
        } else {
            None
        }
    }
}
