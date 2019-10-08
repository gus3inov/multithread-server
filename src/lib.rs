use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

enum Message {
    NewJob(Job),
    Terminate,
}

pub struct ThreadPool {
    inner: Arc<ThreadPoolInner>,
}

struct ThreadPoolInner {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message>,
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

impl Clone for ThreadPool {
    fn clone(&self) -> Self {
        ThreadPool {
            inner: self.inner.clone(),
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for _ in &self.inner.workers {
            self.inner
                .sender
                .send(Message::Terminate)
                .expect("Failed when sending a message");
        }

        for worker in &self.inner.workers {
            match worker.inner.lock() {
                Ok(mut worker_inner) => {
                    println!("Shutting down worker {}", worker_inner.id);

                    if let Some(thread) = worker_inner.thread.take() {
                        thread
                            .join()
                            .expect("Couldn't join on the associated thread");
                    }
                }
                Err(_) => panic!("Failed to drop worker"),
            };
        }
    }
}

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}
type Job = Box<FnBox + Send + 'static>;

struct WorkerInner {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

struct Worker {
    inner: Arc<Mutex<WorkerInner>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv().unwrap();

            match message {
                Message::NewJob(job) => {
                    println!("Worker {} got a job; executing.", id);

                    job.call_box();
                }
                Message::Terminate => {
                    println!("Worker {} was told to terminate.", id);

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
