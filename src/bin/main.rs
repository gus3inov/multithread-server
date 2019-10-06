extern crate threadpool;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use threadpool::ThreadPool;

fn main() {
    if let Ok(pool) = ThreadPool::new(1) {
        let (tx, rx) = mpsc::sync_channel(0);

        pool.clone().execute(move || {
            tx.send("hey").unwrap();
        });

        assert_eq!("hey", rx.recv().unwrap());
    } else {
        assert!(false, "pool should be create")
    }
}
