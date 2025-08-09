use alloc::sync::Arc;

use axtask::{CurrentTask, TaskExtRef, current};

use xprocess::{Process, Thread};

use crate::{
    mm::XUserSpace,
    task::{XProcess, XTaskExt, XThread},
};

pub fn with_current<T, F>(f: F) -> T
where
    F: FnOnce(&CurrentTask) -> T,
{
    f(&current())
}

pub fn with_xtask<T, F>(f: F) -> T
where
    F: FnOnce(&XTaskExt) -> T,
{
    f(current().task_ext())
}

pub fn with_thread<T, F>(f: F) -> T
where
    F: FnOnce(&Arc<Thread>) -> T,
{
    f(current().task_ext().thread_ref())
}

pub fn with_process<T, F>(f: F) -> T
where
    F: FnOnce(&Arc<Process>) -> T,
{
    f(current().task_ext().process_ref())
}

pub fn with_xthread<T, F>(f: F) -> T
where
    F: FnOnce(&XThread) -> T,
{
    f(current().task_ext().xthread_ref())
}

pub fn with_xprocess<T, F>(f: F) -> T
where
    F: FnOnce(&XProcess) -> T,
{
    f(current().task_ext().xprocess_ref())
}

pub fn with_uspace<T, F>(f: F) -> T
where
    F: FnOnce(&XUserSpace) -> T,
{
    f(current().task_ext().xprocess_ref().uspace())
}
