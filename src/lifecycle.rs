pub const LIFECYCLE_BITS: usize = 3;
pub const LIFECYCLE_MASK: usize = 7;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Lifecycle {
    Running = 0,
    Stop = 1,
    Tidying = 2,
    Terminated = 3,
}

impl Lifecycle {
    pub fn from_usize(val: usize) -> Lifecycle {
        use self::Lifecycle::*;

        match val {
            0 => Running,
            1 => Stop,
            2 => Tidying,
            3 => Terminated,
            _ => panic!("unexpected state value"),
        }
    }
}
