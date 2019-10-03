# Thread Pool

A multithreaded server that creates a thread pool with a maximum of 4 threads when connected.

## ThreadPool Module

When connecting thread pool is filled with a certain number of workers, which is specified in the constructor. In the context of each worker, certain job is done. Job is a closure that is passed to the execute thread pool. Workers communicate with each other through channels

### Usage

```rust
extern crate multithread;

use multithread::ThreadPool;

fn main () {
    let pool = ThreadPool::new(4).unwrap();

    pool.execute(|| {
        // some closure body ...
    });
}


```

### API

- #### _prop_ `workers` - vector of workers. Worker has a field `id` and `thread` in which executing closure
- #### _prop_ `sender` - message sender

* #### _fn_ `new` - constructor takes one argument of type `usize`, which specifies the maximum number of workers to create. The result of executing the constructor is a value of type `Result<ThreadPool, &'static str>`,depending on the first parameter passed

- #### _fn_ `execute` - The first argument is a closure of type `FnOnce() + Send`. When the call sends messages, receives the message and executes it.
