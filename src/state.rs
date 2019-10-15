use crate::{lifecycle::{Lifecycle, LIFECYCLE_BITS, LIFECYCLE_MASK}};

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct State {
    state: usize,
}

impl State {
    pub fn load(num: usize) -> State {
        State {
            state: num
        }
    }

    pub fn of(lifecycle: Lifecycle) -> State {
        State {
            state: lifecycle as usize
        }
    }

    pub fn lifecycle(&self) -> Lifecycle {
        Lifecycle::from_usize(self.state & LIFECYCLE_MASK)
    }

    pub fn with_lifecycle(&self, lifecycle: Lifecycle) -> State {
        let state = self.state & !LIFECYCLE_MASK | lifecycle as usize;
        State {
            state: state
        }
    }

    pub fn worker_count(&self) -> usize {
        self.state >> LIFECYCLE_BITS
    }

    pub fn is_terminated(&self) -> bool {
        self.lifecycle() == Lifecycle::Terminated
    }

    pub fn as_usize(&self) -> usize {
        self.state
    }
}
