extern crate multix;

use multix::ThreadPool;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

#[test]
fn one_thread() {
    let (sender, _) = ThreadPool::fixed_size(1);
    let (tx, rx) = mpsc::sync_channel(0);

    sender
        .send(move || {
            tx.send("lol").unwrap();
        })
        .unwrap();

    assert_eq!("lol", rx.recv().unwrap());
}

#[test]
fn two_thread() {
    let (sender, _) = ThreadPool::fixed_size(2);
    let (tx, rx) = mpsc::sync_channel(0);

    for _ in 0..2 {
        let tx = tx.clone();
        sender
            .send(move || {
                tx.send("lol").unwrap();
                thread::sleep(Duration::from_millis(500));

                tx.send("kek").unwrap();
                thread::sleep(Duration::from_millis(500));
            })
            .unwrap();
    }

    for &msg in ["lol", "lol", "kek", "kek"].iter() {
        assert_eq!(msg, rx.recv().unwrap());
    }
}

#[test]
fn clone_pool() {
    let (sender, _) = ThreadPool::fixed_size(1);
    let (tx, rx) = mpsc::sync_channel(1);

    sender
        .clone()
        .send(move || {
            tx.send("hey").unwrap();
        })
        .unwrap();

    assert_eq!("hey", rx.recv().unwrap());
}

#[test]
fn threads_shutdown_drop() {
    let (sender, pool) = ThreadPool::single_thread();
    let atom = Arc::new(AtomicUsize::new(5));

    for _ in 0..10 {
        let atom = atom.clone();
        sender
            .send(move || {
                atom.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
    }

    drop(sender);

    assert!(pool.is_terminating() || pool.is_terminated());

    pool.await_termination();

    assert_eq!(15, atom.load(Ordering::SeqCst));
    assert!(pool.is_terminated());
}

#[test]
fn two_thread_job_on() {
    let (sender, _) = ThreadPool::fixed_size(2);
    let (tx, rx) = mpsc::sync_channel(0);

    for _ in 0..4 {
        let tx = tx.clone();

        sender
            .send(move || {
                tx.send("lol").unwrap();
                thread::sleep(Duration::from_millis(500));

                tx.send("kek").unwrap();
                thread::sleep(Duration::from_millis(500));
            })
            .unwrap();
    }

    for &msg in ["lol", "lol", "kek", "kek", "lol", "lol", "kek", "kek"].iter() {
        assert_eq!(msg, rx.recv().unwrap());
    }
}
