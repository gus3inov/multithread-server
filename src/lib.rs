mod job;

use std::sync::{mpsc, Arc, Mutex};
use std::{thread, fmt};
pub use job::{Job, JobBox};

enum Message {
    NewJob(Job),
    Terminate,
}

#[derive(Debug)]
struct Config {
    core_size: usize,
    max_size: usize,
    stack_size: Option<usize>,
    mount: Option<Arc<Fn() + Send + Sync>>,
    leave: Option<Arc<Fn() + Send + Sync>>,
}

pub struct ThreadPool<T> {
    inner: Arc<Inner<T>>,
}

#[derive(Debug)]
pub struct TPBuilder {
    config: Config,
    max_size_workers: usize,
}

pub struct Sender<T> {
    tx: mpsc::Sender<T>,
    inner: Arc<Inner<T>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Result<ThreadPool, &'static str> {
        if size <= 0 {
            Err("Size should be more then 0")
        } else {
            let mut workers = Vec::with_capacity(size);
            let (sender, reciever) = mpsc::channel();
            let reciever = Arc::new(Mutex::new(reciever));

            for id in 0..size {
                workers.push(Worker::new(id, reciever.clone()));
            }

            Ok(ThreadPool {
                inner: Arc::new(ThreadPoolInner { workers, sender }),
            })
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.inner
            .sender
            .send(Message::NewJob(job))
            .expect("Failed when sending a message");
    }
}

struct Inner<T> {
    config: Config,
    rx: mpsc::Receiver<T>,
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
           .field("core_size", &self.core_size)
           .field("max_size", &self.max_size)
           .field("stack_size", &self.stack_size)
           .field("mount", if self.mount.is_some() { SOME } else { NONE })
           .field("leave", if self.leave.is_some() { SOME } else { NONE })
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


// impl Drop for ThreadPool {
//     fn drop(&mut self) {
//         for _ in &self.inner.workers {
//             self.inner
//                 .sender
//                 .send(Message::Terminate)
//                 .expect("Failed when sending a message");
//         }

//         for worker in &self.inner.workers {
//             match worker.inner.lock() {
//                 Ok(mut worker_inner) => {
//                     println!("\nShutting down worker {}", worker_inner.id);

//                     if let Some(thread) = worker_inner.thread.take() {
//                         thread
//                             .join()
//                             .expect("Couldn't join on the associated thread");
//                     }
//                 }
//                 Err(_) => panic!("Failed to drop worker"),
//             };
//         }
//     }
// }

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();

            match message {
                Message::NewJob(job) => {
                    println!("\nWorker {} got a job; executing.", id);

                    job.call_box();
                }
                Message::Terminate => {
                    println!("\nWorker {} was told to terminate.", id);

                    break;
                }
            }
        });

        Worker {
            inner: Arc::new(Mutex::new(WorkerInner {
                id,
                thread: Some(thread),
            })),
        }
    }
}
