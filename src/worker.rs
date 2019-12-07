use self::core::Inner;
use crate::{core, job};
use crossbeam_channel::{Receiver, RecvTimeoutError, TryRecvError};
use job::Job;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct Worker<T> {
    pub rx: Receiver<T>,
    pub inner: Arc<Inner<T>>,
}

impl<T: Job> Worker<T> {
    pub fn spawn(self, initial_job: Option<T>) {
        let mut b = thread::Builder::new();

        {
            let c = &self.inner.config;

            if let Some(stack_size) = c.stack_size {
                b = b.stack_size(stack_size);
            }
        }

        b.spawn(move || self.run(initial_job)).unwrap();
    }

    fn run(mut self, mut initial_job: Option<T>) {
        use std::panic::{self, AssertUnwindSafe};

        self.inner.config.mount.as_ref().map(|f| f());

        while let Some(job) = self.next_job(initial_job.take()) {
            let _ = panic::catch_unwind(AssertUnwindSafe(move || job.call()));
        }
    }

    fn next_job(&mut self, mut job: Option<T>) -> Option<T> {
        let state = self.inner.state.load();

        let mut timed_out = false;
        let size = self.inner.config.size;

        loop {
            if state.is_stoped() {
                self.inner.config.unmount.as_ref().map(|f| f());

                self.decrement_worker_count();

                return None;
            }

            if job.is_some() {
                break;
            }

            let wc = state.worker_count();

            let timeout = if wc > size {
                self.inner.config.timeout
            } else {
                None
            };

            if timeout.is_some() && timed_out {
                if wc > 1 || self.rx.len() == 0 {
                    if self.inner.state.compare_and_dec_worker_count(state) {
                        self.inner.config.unmount.as_ref().map(|f| f());

                        return None;
                    }

                    continue;
                }
            }

            match self.recv_job(timeout) {
                Ok(t) => {
                    job = Some(t);
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.inner.config.unmount.as_ref().map(|f| f());

                    self.decrement_worker_count();

                    return None;
                }
                Err(RecvTimeoutError::Timeout) => {
                    timed_out = true;
                }
            }
        }

        job
    }

    pub fn is_disconnected(&self) -> bool {
        match self.rx.try_recv() {
            Err(TryRecvError::Disconnected) => true,
            _ => false,
        }
    }

    fn recv_job(&self, timeout: Option<Duration>) -> Result<T, RecvTimeoutError> {
        match timeout {
            Some(timeout) => self.rx.recv_timeout(timeout),
            None => self.rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        }
    }

    fn decrement_worker_count(&self) {
        let state = self.inner.state.fetch_dec_worker_count();

        if state.worker_count() == 1 && self.is_disconnected() {
            self.inner.finalize_instance();
        }
    }
}
