use std::sync::atomic::{AtomicUsize, Ordering};
use crate::{state::{State}, lifecycle::{Lifecycle, LIFECYCLE_BITS}};

pub struct AtomicState {
    atomic: AtomicUsize,
}

pub const CAPACITY: usize = (1 << (32 - 3)) - 1;

impl AtomicState {
    pub fn new(lifecycle: Lifecycle) -> AtomicState {
        let i = State::of(lifecycle).as_usize();

        AtomicState {
            atomic: AtomicUsize::new(i),
        }
    }

    pub fn load(&self) -> State {
        let num = self.atomic.load(Ordering::SeqCst);

        State::load(num)
    }

    fn compare_and_swap(&self, expect: State, val: State) -> State {
        let actual = self.atomic.compare_and_swap(expect.as_usize(), val.as_usize(), Ordering::SeqCst);

        State::load(actual)
    }

    pub fn compare_and_inc_worker_count(&self, expect: State) -> Result<State, State> {
        let expect_usize = expect.as_usize();
        let next_usize = expect_usize + (1 << LIFECYCLE_BITS);
        let actual_usize = self.atomic.compare_and_swap(expect_usize, next_usize, Ordering::SeqCst);

        if actual_usize == expect_usize {
            Ok(expect)
        } else {
            Err(State::load(actual_usize))
        }
    }

    pub fn compare_and_dec_worker_count(&self, expect: State) -> bool {
        if expect.worker_count() == 0 {
            panic!("Workers empty")
        }

        let num = expect.as_usize();
        self.atomic.compare_and_swap(num, num - (1 << LIFECYCLE_BITS), Ordering::SeqCst) == num
    }

    pub fn fetch_dec_worker_count(&self) -> State {
        let prev = self.atomic.fetch_sub(1 << LIFECYCLE_BITS, Ordering::SeqCst);
        State::load(prev)
    }

    fn try_transition_to_lifecycle(&self, lifecycle: Lifecycle) -> bool {
        let mut state = self.load();

        loop {
            if state.lifecycle() >= lifecycle {
                return false;
            }

            let next = state.with_lifecycle(lifecycle);
            let actual = self.compare_and_swap(state, next);

            if state == actual {
                return true;
            }

            state = actual;
        }
    }

    pub fn try_transition_to_stop(&self) -> bool {
        self.try_transition_to_lifecycle(Lifecycle::Stop)
    }

    pub fn try_transition_to_tidying(&self) -> bool {
        self.try_transition_to_lifecycle(Lifecycle::Tidying)
    }

    pub fn transition_to_terminated(&self) {
        let mut state = self.load();

        loop {
            if state.lifecycle() != Lifecycle::Tidying {
                panic!();
            }

            let next = state.with_lifecycle(Lifecycle::Terminated);
            let actual = self.compare_and_swap(state, next);

            if state == actual {
                return;
            }

            state = actual;
        }
    }
}
