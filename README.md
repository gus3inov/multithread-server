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

- #### _prop_ `workers` - вектор воркеров. Сам воркер имеет `id` и `thread` в котором выполняется замыкание
- #### _prop_ `sender` - отправитель сообщений

* #### _fn_ `new` - конструктор принимает один аргумент типа `usize`, который указывает максимальное количество создаваемых воркеров. Результат выполнения конструктора значение типа `Result<ThreadPool, &'static str>` в зависимости от переданного первого параметра

* #### _fn_ `execute` - Первым аргументом принимает замыкание типа `FnOnce() + Send`. При вызове выполняет отправку сообщений, свободный воркер принимает сообщение и выполняет его (замыкание)
