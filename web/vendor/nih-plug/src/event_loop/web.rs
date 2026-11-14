//! Lemon Studio web build: wasm32 has one thread, so every task runs where it is scheduled.

use std::sync::Weak;

use super::{EventLoop, MainThreadExecutor};

pub(crate) struct WebEventLoop<T, E> {
    executor: Weak<E>,
    _task: std::marker::PhantomData<fn(T)>,
}

impl<T, E> EventLoop<T, E> for WebEventLoop<T, E>
where
    T: Send + 'static,
    E: MainThreadExecutor<T> + 'static,
{
    fn new_and_spawn(executor: Weak<E>) -> Self {
        Self { executor, _task: std::marker::PhantomData }
    }

    fn schedule_gui(&self, task: T) -> bool {
        match self.executor.upgrade() {
            Some(e) => {
                e.execute(task, true);
                true
            }
            None => false,
        }
    }

    fn schedule_background(&self, task: T) -> bool {
        match self.executor.upgrade() {
            Some(e) => {
                e.execute(task, false);
                true
            }
            None => false,
        }
    }

    fn is_main_thread(&self) -> bool {
        true
    }
}
