use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, MAX_NUM_RES, MAX_NUM_THREAD};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

fn is_res_safe(allocation: &[[usize; MAX_NUM_RES]; MAX_NUM_THREAD], available: &[usize; MAX_NUM_RES], needed: &[[usize; MAX_NUM_RES]; MAX_NUM_THREAD]) -> bool {
    let mut working = available.clone();
    let mut is_finish = [false; MAX_NUM_THREAD];

    let mut is_progressed = true;
    
    loop {
        for i in 0..MAX_NUM_THREAD {
            if !is_finish[i] {
                let mut can_allocate = true;
                for j in 0..MAX_NUM_RES {
                    

                    if needed[i][j] > working[j] {
                        can_allocate = false;
                        break;

                    }
                }
                if can_allocate {
                    for j in 0..MAX_NUM_RES {                        
                        working[j] += allocation[i][j];
                    }
                    is_finish[i] = true;
                    is_progressed = true;
                    break;
                }
            }
        }
        if !is_progressed {
            break;
        }
        is_progressed = false;
    }
    for finished in is_finish {
        if !finished {
            return false;
        }
    }
    true
}

/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.available[id] = 1;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        let id = process_inner.mutex_list.len() as isize - 1;
        process_inner.available[id as usize] = 1;
        id as isize
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;

    process_inner.needed[tid][mutex_id] += 1;

    let allocaton = process_inner.allocation.clone();
    let available = process_inner.available.clone();
    let needed = process_inner.needed.clone();

    if !is_res_safe(&allocaton, &available, &needed) && process_inner.if_dead_detect {
        process_inner.needed[tid][mutex_id] -= 1;
        return  -0xDEAD;
    }

    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);

    mutex.lock();

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.needed[tid][mutex_id] -= 1;
    process_inner.available[mutex_id] -= 1;
    process_inner.allocation[tid][mutex_id] += 1;

    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let tid = current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(), tid
        
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());

    process_inner.available[mutex_id] += 1;
    process_inner.allocation[tid][mutex_id] -= 1;
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.available[id] = res_count;
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        let id = process_inner.semaphore_list.len() - 1;
        process_inner.available[id] = res_count;
        id
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {    
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    process_inner.available[sem_id] += 1;
    process_inner.allocation[tid][sem_id] -= 1;
    drop(process_inner);
    drop(process);
    sem.up();

    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let tid = current_task()
    .unwrap()
    .inner_exclusive_access()
    .res
    .as_ref()
    .unwrap()
    .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    
    process_inner.needed[tid][sem_id] += 1;
    let allocaton = process_inner.allocation.clone();
    let available = process_inner.available.clone();
    let needed = process_inner.needed.clone();

    if !is_res_safe(&allocaton, &available, &needed) && process_inner.if_dead_detect {
        process_inner.needed[tid][sem_id] -= 1;
        println!("dead lock: {}", tid);
        return  -0xDEAD;
    }
    drop(process_inner);
    drop(process);

    sem.down();

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.needed[tid][sem_id] -= 1;
    process_inner.available[sem_id] -= 1;
    process_inner.allocation[tid][sem_id] += 1;
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if _enabled == 1 {
        process_inner.if_dead_detect = true;
    } else {
        process_inner.if_dead_detect = false;
    }
    0
}
