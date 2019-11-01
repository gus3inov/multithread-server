use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use std::{fmt, usize};

use crate::{atomic, job, lifecycle, worker};
use atomic::{AtomicState, CAPACITY};
use job::{Job, JobBox};
use lifecycle::Lifecycle;
use num_cpus;
use two_lock_queue::{self as mpmc, SendError, SendTimeoutError, TrySendError};
use worker::Worker;

const QUEUE_CAPACITY: usize = 64 * 1_024;

pub struct ThreadPool<T> {
    inner: Arc<Inner<T>>,
}

#[derive(Debug)]
pub struct TPBuilder {
    instance: Config,

    queue_capacity: usize,
}

pub struct Config {
    pub size: usize,
    pub timeout: Option<Duration>,
    pub stack_size: Option<usize>,
    pub mount: Option<Arc<Fn() + Send + Sync>>,
    pub unmount: Option<Arc<Fn() + Send + Sync>>,
}

pub struct Sender<T> {
    tx: mpmc::Sender<T>,
    inner: Arc<Inner<T>>,
}

pub struct Inner<T> {
    pub state: AtomicState,
    pub rx: mpmc::Receiver<T>,
    pub termination_mutex: Mutex<()>,
    pub termination_signal: Condvar,
    pub config: Config,
}

impl fmt::Debug for Config {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        const SOME: &'static &'static str = &"Some(_)";
        const NONE: &'static &'static str = &"None";

        fmt.debug_struct("ThreadPool")
            .field("size", &self.size)
            .field("timeout", &self.timeout)
            .field("stack_size", &self.stack_size)
            .field("mount", if self.mount.is_some() { SOME } else { NONE })
            .field("unmount", if self.unmount.is_some() { SOME } else { NONE })
            .finish()
    }
}

impl TPBuilder {
    pub fn new() -> TPBuilder {
        let num_cpus = num_cpus::get();

        TPBuilder {
            instance: Config {
                size: num_cpus,
                timeout: None,
                stack_size: None,
                mount: None,
                unmount: None,
            },
            queue_capacity: QUEUE_CAPACITY,
        }
    }

    pub fn size(mut self, val: usize) -> Self {
        self.instance.size = val;
        self
    }

    pub fn timeout(mut self, val: Duration) -> Self {
        self.instance.timeout = Some(val);
        self
    }

    pub fn queue_capacity(mut self, val: usize) -> Self {
        self.queue_capacity = val;
        self
    }

    pub fn stack_size(mut self, val: usize) -> Self {
        self.instance.stack_size = Some(val);
        self
    }

    pub fn mount<F>(mut self, f: Option<F>) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        if let Some(f) = f {
            self.instance.mount = Some(Arc::new(f));
            self
        } else {
            self
        }
    }

    pub fn unmount<F>(mut self, f: Option<F>) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        if let Some(f) = f {
            self.instance.unmount = Some(Arc::new(f));
            self
        } else {
            self
        }
    }

    pub fn build<T: Job>(self) -> (Sender<T>, ThreadPool<T>) {
        assert!(self.instance.size >= 1, "at least one thread required");

        let (tx, rx) = mpmc::channel(self.queue_capacity);
        let termination_mutex = Mutex::new(());
        let termination_signal = Condvar::new();

        let inner = Arc::new(Inner {
            state: AtomicState::new(Lifecycle::Running),
            rx,
            termination_mutex,
            termination_signal,
            config: self.instance,
        });

        let sender = Sender {
            tx: tx,
            inner: inner.clone(),
        };

        let pool = ThreadPool { inner: inner };

        (sender, pool)
    }
}

impl<T: Job> ThreadPool<T> {
    pub fn new(size: usize) -> (Sender<T>, ThreadPool<T>) {
        TPBuilder::new()
            .size(size)
            .queue_capacity(usize::MAX)
            .build()
    }

    pub fn new_with_hooks<F>(
        size: usize,
        mount: Option<F>,
        unmount: Option<F>,
    ) -> (Sender<T>, ThreadPool<T>)
    where
        F: Sized + Fn() + Send + Sync + 'static,
    {
        TPBuilder::new()
            .size(size)
            .queue_capacity(usize::MAX)
            .mount(mount)
            .unmount(unmount)
            .build()
    }

    pub fn single_thread() -> (Sender<T>, ThreadPool<T>) {
        TPBuilder::new().size(1).queue_capacity(usize::MAX).build()
    }

    pub fn prestart_core_thread(&self) -> bool {
        if !self.inner.is_workers_overflow() {
            self.inner.add_worker(None, &self.inner).is_ok()
        } else {
            false
        }
    }

    pub fn prestart_core_threads(&self) {
        while self.prestart_core_thread() {}
    }

    pub fn shutdown(&self) {
        self.inner.rx.close();
    }

    pub fn shutdown_now(&self) {
        self.inner.rx.close();

        if self.inner.state.try_transition_to_stop() {
            loop {
                match self.inner.rx.recv() {
                    Err(_) => return,
                    Ok(_) => {}
                }
            }
        }
    }

    pub fn is_terminating(&self) -> bool {
        !self.inner.rx.is_open() && !self.is_terminated()
    }

    pub fn is_terminated(&self) -> bool {
        self.inner.state.load().is_terminated()
    }

    pub fn await_termination(&self) {
        let mut lock = self.inner.termination_mutex.lock().unwrap();

        while !self.inner.state.load().is_terminated() {
            lock = self.inner.termination_signal.wait(lock).unwrap();
        }
    }

    pub fn size(&self) -> usize {
        self.inner.state.load().worker_count()
    }

    pub fn queued(&self) -> usize {
        self.inner.rx.len()
    }
}

impl<T> Clone for ThreadPool<T> {
    fn clone(&self) -> Self {
        ThreadPool {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Job> fmt::Debug for ThreadPool<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ThreadPool").finish()
    }
}

impl<T: Job> Sender<T> {
    pub fn send(&self, job: T) -> Result<(), SendError<T>> {
        match self.try_send(job) {
            Ok(_) => Ok(()),
            Err(TrySendError::Disconnected(job)) => Err(SendError(job)),
            Err(TrySendError::Full(job)) => self.tx.send(job),
        }
    }

    pub fn send_timeout(&self, job: T, timeout: Duration) -> Result<(), SendTimeoutError<T>> {
        match self.try_send(job) {
            Ok(_) => Ok(()),
            Err(TrySendError::Disconnected(job)) => Err(SendTimeoutError::Disconnected(job)),
            Err(TrySendError::Full(job)) => self.tx.send_timeout(job, timeout),
        }
    }

    pub fn try_send(&self, job: T) -> Result<(), TrySendError<T>> {
        match self.tx.try_send(job) {
            Ok(_) => {
                if !self.inner.is_workers_overflow() {
                    let _ = self.inner.add_worker(None, &self.inner);
                }

                Ok(())
            }
            Err(TrySendError::Disconnected(job)) => {
                return Err(TrySendError::Disconnected(job));
            }
            Err(TrySendError::Full(job)) => match self.inner.add_worker(Some(job), &self.inner) {
                Ok(_) => return Ok(()),
                Err(job) => return Err(TrySendError::Full(job.unwrap())),
            },
        }
    }
}

impl Sender<Box<JobBox>> {
    pub fn send_fn<F>(&self, job: F) -> Result<(), SendError<Box<JobBox>>>
    where
        F: FnOnce() + Send + 'static,
    {
        let job: Box<JobBox> = Box::new(job);
        self.send(job)
    }

    pub fn send_fn_timeout<F>(
        &self,
        job: F,
        timeout: Duration,
    ) -> Result<(), SendTimeoutError<Box<JobBox>>>
    where
        F: FnOnce() + Send + 'static,
    {
        let job: Box<JobBox> = Box::new(job);
        self.send_timeout(job, timeout)
    }

    pub fn try_send_fn<F>(&self, job: F) -> Result<(), TrySendError<Box<JobBox>>>
    where
        F: FnOnce() + Send + 'static,
    {
        let job: Box<JobBox> = Box::new(job);
        self.try_send(job)
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender {
            tx: self.tx.clone(),
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Sender").finish()
    }
}

impl<T: Job> Inner<T> {
    fn add_worker(&self, job: Option<T>, arc: &Arc<Inner<T>>) -> Result<(), Option<T>> {
        let mut state = self.state.load();

        'retry: loop {
            let lifecycle = state.lifecycle();

            if state.is_stoped() {
                return Err(job);
            }

            loop {
                let wc = state.worker_count();

                if wc >= CAPACITY || wc >= self.config.size {
                    return Err(job);
                }

                state = match self.state.compare_and_inc_worker_count(state) {
                    Ok(_) => break 'retry,
                    Err(state) => state,
                };

                if state.lifecycle() != lifecycle {
                    continue 'retry;
                }
            }
        }

        let worker = Worker {
            rx: self.rx.clone(),
            inner: arc.clone(),
        };

        worker.spawn(job);

        Ok(())
    }

    pub fn is_workers_overflow(&self) -> bool {
        let state = self.state.load();

        state.worker_count() >= self.config.size
    }

    pub fn finalize_instance(&self) {
        if self.state.try_transition_to_tidying() {
            self.state.transition_to_terminated();

            self.termination_signal.notify_all();
        }
    }
}
