use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

use axtask::{TaskExtRef, TaskInner, TaskState};

use super::{XProcess, XThread};

pub fn status(task: &TaskInner) -> String {
    let task_ext = task.task_ext();
    let thread = task_ext.thread();
    let process = thread.process();
    // let xthread = XThread::from_thread_static(&thread);
    let xprocess = XProcess::from_process_static(process);

    let state = match task.state() {
        TaskState::Running | TaskState::Ready => "R (Running)",
        TaskState::Blocked => "S (Sleeping)",
        TaskState::Exited => "Z (Exited)",
    };
    let name = xprocess
        .exe_path
        .read()
        .split('/')
        .next_back()
        .unwrap_or("unknown")
        .to_string();
    let groups = xprocess.sup_group();
    let groups_str = groups
        .lock()
        .iter()
        .map(|g| g.to_string())
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "Name:\t{}\n\
        Umask:\t0022\n\
        State:\t{}\n\
        Tgid:\t{}\n\
        Ngid:\t0\n\
        Pid:\t{}\n\
        PPid:\t{}\n\
        TracerPid:\t0\n\
        Uid:\t{}\t{}\t{}\t{}\n\
        Gid:\t{}\t{}\t{}\t{}\n\
        FDSize:\t256\n\
        Groups:\t{}\n\
        Threads:\t{}\n\
        SigQ:\t0/31854\n\
        SigPnd:\t0000000000000000\n\
        ShdPnd:\t0000000000000000\n\
        SigBlk:\t0000000000000000\n\
        SigIgn:\t0000000000000000\n\
        SigCgt:\t0000000000000000\n\
        CapInh:\t0000000000000000\n\
        CapPrm:\t0000000000000000\n\
        CapEff:\t0000000000000000\n\
        CapBnd:\t0000000000000000\n\
        Seccomp:\t0\n\
        Cpus_allowed:\tf\n\
        Cpus_allowed_list:\t0-3\n\
        Mems_allowed:\t1\n\
        Mems_allowed_list:\t0\n\
        voluntary_ctxt_switches:\t1\n\
        nonvoluntary_ctxt_switches:\t0",
        name,
        state,
        process.pid(),
        thread.tid(),
        process.parent().map(|p| p.pid()).unwrap_or(0),
        xprocess.credentials.uid(),
        xprocess.credentials.euid(),
        xprocess.credentials.suid(),
        xprocess.credentials.fsuid(),
        xprocess.credentials.gid(),
        xprocess.credentials.egid(),
        xprocess.credentials.sgid(),
        xprocess.credentials.fsgid(),
        groups_str,
        process.threads().len(),
    )
}

#[derive(Default)]
pub struct ProcStat {
    pub pid: u32,
    pub comm: String,
    pub state: char,
    pub ppid: u32,
    pub pgrp: u32,
    pub session: u32,
    pub tty_nr: u32,
    pub tpgid: u32,
    pub flags: u32,
    pub minflt: u64,
    pub cminflt: u64,
    pub majflt: u64,
    pub cmajflt: u64,
    pub utime: u64,
    pub stime: u64,
    pub cutime: u64,
    pub cstime: u64,
    pub priority: u32,
    pub nice: u32,
    pub num_threads: u32,
    pub itrealvalue: u32,
    pub starttime: u64,
    pub vsize: u64,
    pub rss: i64,
    pub rsslim: u64,
    pub start_code: u64,
    pub end_code: u64,
    pub start_stack: u64,
    pub kstk_esp: u64,
    pub kstk_eip: u64,
    pub signal: u32,
    pub blocked: u32,
    pub sigignore: u32,
    pub sigcatch: u32,
    pub wchan: u64,
    pub nswap: u64,
    pub cnswap: u64,
    pub exit_signal: u8,
    pub processor: u32,
    pub rt_priority: u32,
    pub policy: u32,
    pub delayacct_blkio_ticks: u64,
    pub guest_time: u64,
    pub cguest_time: u64,
    pub start_data: u64,
    pub end_data: u64,
    pub start_brk: u64,
    pub arg_start: u64,
    pub arg_end: u64,
    pub env_start: u64,
    pub env_end: u64,
    pub exit_code: i32,
}

pub fn stat(task: &TaskInner) -> String {
    let task_ext = task.task_ext();
    let thread = task_ext.thread();
    let process = thread.process();
    let xthread = XThread::from_thread_static(&thread);
    let xprocess = XProcess::from_process_static(process);

    let state = match task.state() {
        TaskState::Running | TaskState::Ready => 'R',
        TaskState::Blocked => 'S',
        TaskState::Exited => 'Z',
    };

    let comm = xprocess
        .exe_path
        .read()
        .split('/')
        .next_back()
        .unwrap_or("unknown")
        .to_string();

    let stat = ProcStat {
        pid: thread.tid(),
        comm,
        state,
        ppid: process.parent().map(|p| p.pid()).unwrap_or(0),
        pgrp: process.group().pgid(),
        session: process.group().session().sid(),
        priority: xthread.get_priority() as u32,
        num_threads: process.threads().len() as u32,
        rsslim: u64::MAX,
        exit_signal: 17,
        rt_priority: xthread.get_priority() as u32,
        policy: xthread.get_policy(),
        exit_code: task.exit_code(),
        ..Default::default()
    };

    format!(
        "{} ({}) {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        stat.pid,
        stat.comm,
        stat.state,
        stat.ppid,
        stat.pgrp,
        stat.session,
        stat.tty_nr,
        stat.tpgid,
        stat.flags,
        stat.minflt,
        stat.cminflt,
        stat.majflt,
        stat.cmajflt,
        stat.utime,
        stat.stime,
        stat.cutime,
        stat.cstime,
        stat.priority,
        stat.nice,
        stat.num_threads,
        stat.itrealvalue,
        stat.starttime,
        stat.vsize,
        stat.rss,
        stat.rsslim,
        stat.start_code,
        stat.end_code,
        stat.start_stack,
        stat.kstk_esp,
        stat.kstk_eip,
        stat.signal,
        stat.blocked,
        stat.sigignore,
        stat.sigcatch,
        stat.wchan,
        stat.nswap,
        stat.cnswap,
        stat.exit_signal,
        stat.processor,
        stat.rt_priority,
        stat.policy,
        stat.delayacct_blkio_ticks,
        stat.guest_time,
        stat.cguest_time,
        stat.start_data,
        stat.end_data,
        stat.start_brk,
        stat.arg_start,
        stat.arg_end,
        stat.env_start,
        stat.env_end,
        stat.exit_code,
    )
}
