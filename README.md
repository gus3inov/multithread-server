# Thread Pool

Thread pool crate to execute a certain number of jobs on a fixed set of worker threads.

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

* #### _fn_ `new` - конструктор принимает один аргумент типа `usize`, который указывает максимальное количество создаваемых воркеров. Результат выполнения конструктора значение типа `Result<ThreadPool, &'static str>` в зависимости от переданного первого параметра

constructor takes one argument of type `usize`, which specifies the maximum number of workers to create. The result of executing the constructor is a value of type `Result<ThreadPool, &'static str>`,depending on the first parameter passed

- #### _fn_ `execute` - Первым аргументом принимает замыкание типа `FnOnce() + Send`. При вызове выполняет отправку сообщений, свободный воркер принимает сообщение и выполняет его (замыкание)
  The first argument is a closure of type `FnOnce() + Send`. When the call sends messages, receives the message and executes it.
