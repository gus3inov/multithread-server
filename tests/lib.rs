extern crate threadpool;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use threadpool::ThreadPool;

#[test]
pub fn thread_pool_size_zero() {
    let pool = ThreadPool::new(0);
    match pool {
        Ok(_) => assert!(false, "pool should not create if size zero"),
        Err(_) => assert!(true),
    }
}

#[test]
fn one_thread() {
    if let Ok(pool) = ThreadPool::new(1) {
        let (tx, rx) = mpsc::sync_channel(0);

        pool.execute(move || {
            tx.send("lol").unwrap();
        });

        assert_eq!("lol", rx.recv().unwrap());
    } else {
        assert!(false, "pool should be create")
    }
}

#[test]
fn two_thread() {
    if let Ok(pool) = ThreadPool::new(2) {
        let (tx, rx) = mpsc::sync_channel(0);

        for _ in 0..2 {
            let tx = tx.clone();
            pool.execute(move || {
                tx.send("lol").unwrap();
                thread::sleep(Duration::from_millis(500));

                tx.send("kek").unwrap();
                thread::sleep(Duration::from_millis(500));
            });
        }

        for &msg in ["lol", "lol", "kek", "kek"].iter() {
            assert_eq!(msg, rx.recv().unwrap());
        }
    } else {
        assert!(false, "pool should be create")
    }
}

#[test]
fn clone_pool() {
    if let Ok(pool) = ThreadPool::new(1) {
        let (tx, rx) = mpsc::sync_channel(1);

        pool.clone().execute(move || {
            tx.send("hey").unwrap();
        });

        assert_eq!("hey", rx.recv().unwrap());
    } else {
        assert!(false, "pool should be create")
    }
}
