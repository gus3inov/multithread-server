pub trait Job: Send + 'static {
    fn call(self);
}

pub trait JobBox: Send + 'static {
    fn call_box(self: Box<Self>);
}

impl<F> Job for F where F: FnOnce() + Send + 'static {
    fn call(self){
        (self)()
    }
}

impl<T: Sized + Job> JobBox for T {
    fn call_box(self: Box<Self>) {
        (*self).call()
    }
}

impl Job for Box<JobBox> {
    fn call(self: Self) {
        self.call_box()
    }
}
