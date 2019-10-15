pub mod job;
pub mod atomic;
pub mod lifecycle;
pub mod state;

use job::{Job, JobBox};
use atomic::{AtomicState, CAPACITY};
use lifecycle::{Lifecycle};
use state::{State};
use two_lock_queue::{self as mpmc, SendError, SendTimeoutError, TrySendError, RecvTimeoutError};
use num_cpus;

use std::{fmt, thread, usize};
use std::sync::{Arc, Mutex, Condvar};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

pub struct ThreadPool<T> {
    inner: Arc<Inner<T>>,
}

#[derive(Debug)]
pub struct Builder {
    thread_pool: Config,

    work_queue_capacity: usize,
}

struct Config {
    core_pool_size: usize,
    max_pool_size: usize,
    keep_alive: Option<Duration>,
    allow_core_thread_timeout: bool,
    name_prefix: Option<String>,
    stack_size: Option<usize>,
    after_start: Option<Arc<Fn() + Send + Sync>>,
    before_stop: Option<Arc<Fn() + Send + Sync>>,
}

pub struct Sender<T> {
    tx: mpmc::Sender<T>,
    inner: Arc<Inner<T>>,
}

struct Inner<T> {
    state: AtomicState,
    rx: mpmc::Receiver<T>,
    termination_mutex: Mutex<()>,
    termination_signal: Condvar,
    next_thread_id: AtomicUsize,
    config: Config,
}

impl<T> Clone for ThreadPool<T> {
    fn clone(&self) -> Self {
        ThreadPool { inner: self.inner.clone() }
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

impl fmt::Debug for Config {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        const SOME: &'static &'static str = &"Some(_)";
        const NONE: &'static &'static str = &"None";

        fmt.debug_struct("ThreadPool")
           .field("core_pool_size", &self.core_pool_size)
           .field("core_pool_size", &self.core_pool_size)
           .field("max_pool_size", &self.max_pool_size)
           .field("keep_alive", &self.keep_alive)
           .field("allow_core_thread_timeout", &self.allow_core_thread_timeout)
           .field("name_prefix", &self.name_prefix)
           .field("stack_size", &self.stack_size)
           .field("after_start", if self.after_start.is_some() { SOME } else { NONE })
           .field("before_stop", if self.before_stop.is_some() { SOME } else { NONE })
           .finish()
    }
}

impl<T> fmt::Debug for ThreadPool<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ThreadPool").finish()
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Sender").finish()
    }
}

struct Worker<T> {
    rx: mpmc::Receiver<T>,
    inner: Arc<Inner<T>>,
}

impl Builder {
    pub fn new() -> Builder {
        let num_cpus = num_cpus::get();

        Builder {
            thread_pool: Config {
                core_pool_size: num_cpus,
                max_pool_size: num_cpus,
                keep_alive: None,
                allow_core_thread_timeout: false,
                name_prefix: None,
                stack_size: None,
                after_start: None,
                before_stop: None,
            },
            work_queue_capacity: 64 * 1_024,
        }
    }

    pub fn core_pool_size(mut self, val: usize) -> Self {
        self.thread_pool.core_pool_size = val;
        self
    }

    pub fn max_pool_size(mut self, val: usize) -> Self {
        self.thread_pool.max_pool_size = val;
        self
    }

    pub fn keep_alive(mut self, val: Duration) -> Self {
        self.thread_pool.keep_alive = Some(val);
        self
    }

    pub fn allow_core_thread_timeout(mut self) -> Self {
        self.thread_pool.allow_core_thread_timeout = true;
        self
    }

    pub fn work_queue_capacity(mut self, val: usize) -> Self {
        self.work_queue_capacity = val;
        self
    }

    pub fn name_prefix<S: Into<String>>(mut self, val: S) -> Self {
        self.thread_pool.name_prefix = Some(val.into());
        self
    }

    pub fn stack_size(mut self, val: usize) -> Self {
        self.thread_pool.stack_size = Some(val);
        self
    }

    pub fn after_start<F>(mut self, f: F) -> Self
        where F: Fn() + Send + Sync + 'static
    {
        self.thread_pool.after_start = Some(Arc::new(f));
        self
    }

    pub fn before_stop<F>(mut self, f: F) -> Self
        where F: Fn() + Send + Sync + 'static
    {
        self.thread_pool.before_stop = Some(Arc::new(f));
        self
    }

    pub fn build<T: Job>(self) -> (Sender<T>, ThreadPool<T>) {
        assert!(self.thread_pool.core_pool_size >= 1, "at least one thread required");
        assert!(self.thread_pool.core_pool_size <= self.thread_pool.max_pool_size,
                "`core_pool_size` cannot be greater than `max_pool_size`");
        assert!(self.thread_pool.max_pool_size >= self.thread_pool.core_pool_size,
                "`max_pool_size` must be greater or equal to `core_pool_size`");


        let (tx, rx) = mpmc::channel(self.work_queue_capacity);

        let inner = Arc::new(Inner {
            // Thread pool starts in the running state
            state: AtomicState::new(Lifecycle::Running),
            rx: rx,
            termination_mutex: Mutex::new(()),
            termination_signal: Condvar::new(),
            next_thread_id: AtomicUsize::new(1),
            config: self.thread_pool,
        });

        let sender = Sender {
            tx: tx,
            inner: inner.clone(),
        };

        let pool = ThreadPool {
            inner: inner,
        };

        (sender, pool)
    }
}

impl<T: Job> ThreadPool<T> {
    pub fn fixed_size(size: usize) -> (Sender<T>, ThreadPool<T>) {
        Builder::new()
            .core_pool_size(size)
            .max_pool_size(size)
            .work_queue_capacity(usize::MAX)
            .build()
    }

    pub fn single_thread() -> (Sender<T>, ThreadPool<T>) {
        Builder::new()
            .core_pool_size(1)
            .max_pool_size(1)
            .work_queue_capacity(usize::MAX)
            .build()
    }

    pub fn prestart_core_thread(&self) -> bool {
        let wc = self.inner.state.load().worker_count();

        if wc < self.inner.config.core_pool_size {
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

impl<T: Job> Sender<T> {
    pub fn send(&self, job: T) -> Result<(), SendError<T>> {
        match self.try_send(job) {
            Ok(_) => Ok(()),
            Err(TrySendError::Disconnected(job)) => Err(SendError(job)),
            Err(TrySendError::Full(job)) => {
                self.tx.send(job)
            }
        }
    }

    pub fn send_timeout(&self, job: T, timeout: Duration) -> Result<(), SendTimeoutError<T>> {
        match self.try_send(job) {
            Ok(_) => Ok(()),
            Err(TrySendError::Disconnected(job)) => {
                Err(SendTimeoutError::Disconnected(job))
            }
            Err(TrySendError::Full(job)) => {
                self.tx.send_timeout(job, timeout)
            }
        }
    }

    pub fn try_send(&self, job: T) -> Result<(), TrySendError<T>> {
        match self.tx.try_send(job) {
            Ok(_) => {
                let state = self.inner.state.load();

                if state.worker_count() < self.inner.config.core_pool_size {
                    let _ = self.inner.add_worker(None, &self.inner);
                }

                Ok(())
            }
            Err(TrySendError::Disconnected(job)) => {
                return Err(TrySendError::Disconnected(job));
            }
            Err(TrySendError::Full(job)) => {
                match self.inner.add_worker(Some(job), &self.inner) {
                    Ok(_) => return Ok(()),
                    Err(job) => return Err(TrySendError::Full(job.unwrap())),
                }
            }
        }
    }
}

impl Sender<Box<JobBox>> {
    pub fn send_fn<F>(&self, job: F) -> Result<(), SendError<Box<JobBox>>>
        where F: FnOnce() + Send + 'static
    {
        let job: Box<JobBox> = Box::new(job);
        self.send(job)
    }

    pub fn send_fn_timeout<F>(&self, job: F, timeout: Duration)
        -> Result<(), SendTimeoutError<Box<JobBox>>>
        where F: FnOnce() + Send + 'static
    {
        let job: Box<JobBox> = Box::new(job);
        self.send_timeout(job, timeout)
    }

    pub fn try_send_fn<F>(&self, job: F)
        -> Result<(), TrySendError<Box<JobBox>>>
        where F: FnOnce() + Send + 'static
    {
        let job: Box<JobBox> = Box::new(job);
        self.try_send(job)
    }
}

impl<T: Job> Inner<T> {
    fn add_worker(&self, job: Option<T>, arc: &Arc<Inner<T>>)
            -> Result<(), Option<T>> {

        let core = job.is_none();
        let mut state = self.state.load();

        'retry: loop {
            let lifecycle = state.lifecycle();

            if lifecycle >= Lifecycle::Stop {
                return Err(job);
            }

            loop {
                let wc = state.worker_count();

                let target = if core {
                    self.config.core_pool_size
                } else {
                    self.config.max_pool_size
                };

                if wc >= CAPACITY || wc >= target {
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

    fn finalize_thread_pool(&self) {
        if self.state.try_transition_to_tidying() {
            self.state.transition_to_terminated();

            self.termination_signal.notify_all();
        }
    }
}

impl<T: Job> Worker<T> {
    fn spawn(self, initial_job: Option<T>) {
        let mut b = thread::Builder::new();

        {
            let c = &self.inner.config;

            if let Some(stack_size) = c.stack_size {
                b = b.stack_size(stack_size);
            }

            if let Some(ref name_prefix) = c.name_prefix {
                let i = self.inner.next_thread_id.fetch_add(1, Relaxed);
                b = b.name(format!("{}{}", name_prefix, i));
            }
        }

        b.spawn(move || self.run(initial_job)).unwrap();
    }

    fn run(mut self, mut initial_job: Option<T>) {
        use std::panic::{self, AssertUnwindSafe};

        self.inner.config.after_start.as_ref().map(|f| f());

        while let Some(job) = self.next_job(initial_job.take()) {
            let _ = panic::catch_unwind(AssertUnwindSafe(move || job.call()));
        }
    }

    fn next_job(&mut self, mut job: Option<T>) -> Option<T> {
        let state = self.inner.state.load();

        let mut timed_out = false;
        let allow_core_thread_timeout = self.inner.config.allow_core_thread_timeout;
        let core_pool_size = self.inner.config.core_pool_size;

        loop {
            if state.lifecycle() >= Lifecycle::Stop {
                self.inner.config.before_stop.as_ref().map(|f| f());

                self.decrement_worker_count();

                return None;
            }

            if job.is_some() {
                break;
            }

            let wc = state.worker_count();

            // Determine if there is a timeout for receiving the next job
            let timeout = if wc > core_pool_size || allow_core_thread_timeout {
                self.inner.config.keep_alive
            } else {
                None
            };

            if wc > self.inner.config.max_pool_size || (timeout.is_some() && timed_out) {
                if wc > 1 || self.rx.len() == 0 {
                    if self.inner.state.compare_and_dec_worker_count(state) {
                        self.inner.config.before_stop.as_ref().map(|f| f());

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
                    self.inner.config.before_stop.as_ref().map(|f| f());

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

    fn recv_job(&self, timeout: Option<Duration>) -> Result<T, RecvTimeoutError> {
        match timeout {
            Some(timeout) => self.rx.recv_timeout(timeout),
            None => self.rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        }
    }

    fn decrement_worker_count(&self) {
        let state = self.inner.state.fetch_dec_worker_count();

        if state.worker_count() == 1 && !self.rx.is_open() {
            self.inner.finalize_thread_pool();
        }
    }
}
