extern crate multithread;

use multithread::ThreadPool;
use std::sync::mpsc;
use std::time::Duration;
use std::thread;

#[test]
pub fn thread_pool_size_zero() {
    let pool = ThreadPool::new(0);
    
    match pool {
        Ok(_) => assert!(false, "pool should not create if size zero"),
        Err(_) => assert!(true)
    }
}

#[test]
fn one_thread_basic() {
    if let Ok(pool) = ThreadPool::new(1) {
        let (tx, rx) = mpsc::sync_channel(0);

        pool.execute(move || {
            tx.send("hi").unwrap();
        });

        assert_eq!("hi", rx.recv().unwrap());
    } else {
        assert!(false, "pool should be create")
    }
}

#[test]
fn two_thread_basic() {
    if let Ok(pool) = ThreadPool::new(2) {
        let (tx, rx) = mpsc::sync_channel(0);

    for _ in 0..2 {
        let tx = tx.clone();
        pool.execute(move || {
            tx.send("hi").unwrap();
            thread::sleep(Duration::from_millis(500));

            tx.send("bye").unwrap();
            thread::sleep(Duration::from_millis(500));
        });
    }

    for &msg in ["hi", "hi", "bye", "bye"].iter() {
        assert_eq!(msg, rx.recv().unwrap());
    }
    } else {
        assert!(false, "pool should be create")
    }
}
