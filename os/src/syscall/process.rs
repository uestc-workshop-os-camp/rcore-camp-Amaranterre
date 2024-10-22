//! Process management syscalls
use crate::{
    mm::VirtAddr, task::{
        change_program_brk, current_pp_with_va, current_task_mmap, current_task_munmap, exit_current_and_run_next, suspend_current_and_run_next, TaskInfo, TASK_MANAGER
    }, timer::get_time_us
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    // println!("DEBUG in process::current_task_mmap");
    current_task_mmap(_start, _len, _port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    current_task_munmap(_start, _len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    // println!("DEBUG: syscall/process.rs::sys_get_time : after get_time_us");
    unsafe {
        let offset = VirtAddr(ts as usize).page_offset();
        let page = current_pp_with_va(ts as usize);
        // let phisical_ts = &mut page[offset] as *mut u8 as usize as *mut TimeVal;

        let physical_ts = (&mut page[0] as *mut u8).add(offset) as *mut TimeVal;

        *physical_ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    unsafe {    
        let offset = VirtAddr(_ti as usize).page_offset();
        let page = current_pp_with_va(_ti as usize);
        // let phisical_ts = &mut page[offset] as *mut u8 as usize as *mut TimeVal;

        let physical_ti = (&mut page[0] as *mut u8).add(offset) as *mut TaskInfo;

        *physical_ti = TASK_MANAGER.get_task_info();
    }    
    trace!("kernel: sys_task_info");
    0
}
