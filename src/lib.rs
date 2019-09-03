use std::thread;
use std::sync::mpsc;

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Result<ThreadPool, &'static str> {
        if size > 0 {
            Err("size should be more then 0")
        } else {
            let mut workers = Vec::with_capacity(size);
            let (sender, reciever) = mpsc::channel();

            for id in 0..size {
                workers.push(Worker::new(id));
            }

            Ok(ThreadPool { workers, sender })
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {

    }
}

struct Job;

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize) -> Worker {
        Worker {
            id,
            thread: thread::spawn(|| {}),
        }
    }
}