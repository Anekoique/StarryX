use alloc::string::ToString;

use axerrno::{LinuxError, LinuxResult};
use axhal::{
    arch::TrapFrame,
    trap::{SYSCALL, register_trap_handler},
};
use syscalls::Sysno;

use xapi::*;
use xcore::time::{time_stat_from_kernel_to_user, time_stat_from_user_to_kernel};

fn handle_syscall_impl(tf: &mut TrapFrame, sysno: Sysno) -> LinuxResult<isize> {
    match sysno {
        // fs ctl
        Sysno::ioctl => sys_ioctl(tf.arg0() as _, tf.arg1() as _, tf.arg2().into()),
        Sysno::chdir => sys_chdir(tf.arg0().into()),
        #[cfg(target_arch = "x86_64")]
        Sysno::mkdir => sys_mkdir(tf.arg0().into(), tf.arg1() as _),
        Sysno::mkdirat => sys_mkdirat(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::getdents64 => sys_getdents64(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::link => sys_link(tf.arg0().into(), tf.arg1().into()),
        Sysno::linkat => sys_linkat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4() as _,
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::rmdir => sys_rmdir(tf.arg0().into()),
        #[cfg(target_arch = "x86_64")]
        Sysno::unlink => sys_unlink(tf.arg0().into()),
        Sysno::unlinkat => sys_unlinkat(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::getcwd => sys_getcwd(tf.arg0().into(), tf.arg1() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::symlink => sys_symlink(tf.arg0().into(), tf.arg1().into()),
        Sysno::symlinkat => sys_symlinkat(tf.arg0().into(), tf.arg1() as _, tf.arg2().into()),
        #[cfg(target_arch = "x86_64")]
        Sysno::rename => sys_rename(tf.arg0().into(), tf.arg1().into()),
        Sysno::renameat => sys_renameat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3().into(),
        ),
        Sysno::renameat2 => sys_renameat2(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4() as _,
        ),
        Sysno::fchdir => sys_fchdir(tf.arg0() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::mknod => Ok(0),
        Sysno::mknodat => Ok(0),
        Sysno::flock => Ok(0),
        Sysno::fadvise64 => Ok(0),
        Sysno::setxattr => Ok(0),
        Sysno::lsetxattr => Ok(0),
        Sysno::fsetxattr => Ok(0),
        Sysno::removexattr => Ok(0),
        Sysno::lremovexattr => Ok(0),
        Sysno::fremovexattr => Ok(0),
        Sysno::listxattr => Ok(0),
        Sysno::llistxattr => Ok(0),
        Sysno::flistxattr => Ok(0),
        Sysno::getxattr => Err(LinuxError::ENODATA),
        Sysno::fgetxattr => Ok(0),

        // file ops
        Sysno::fchown => sys_fchown(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::fchownat => sys_fchownat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::chmod => sys_chmod(tf.arg0().into(), tf.arg1() as _),
        Sysno::fchmod => sys_fchmod(tf.arg0() as _, tf.arg1() as _),
        Sysno::fchmodat | Sysno::fchmodat2 => sys_fchmodat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::readlink => sys_readlink(tf.arg0().into(), tf.arg1().into(), tf.arg2() as _),
        Sysno::readlinkat => sys_readlinkat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::utime => sys_utime(tf.arg0().into(), tf.arg1().into()),
        #[cfg(target_arch = "x86_64")]
        Sysno::utimes => sys_utimes(tf.arg0().into(), tf.arg1().into()),
        Sysno::utimensat => sys_utimensat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),

        // fd ops
        #[cfg(target_arch = "x86_64")]
        Sysno::open => sys_open(tf.arg0().into(), tf.arg1() as _, tf.arg2() as _),
        Sysno::openat => sys_openat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::close => sys_close(tf.arg0() as _),
        Sysno::dup => sys_dup(tf.arg0() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::dup2 => sys_dup2(tf.arg0() as _, tf.arg1() as _),
        Sysno::dup3 => sys_dup3(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::fcntl => sys_fcntl(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),

        // io
        Sysno::read => sys_read(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::readv => sys_readv(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::write => sys_write(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::writev => sys_writev(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::lseek => sys_lseek(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::truncate => sys_truncate(tf.arg0().into(), tf.arg1() as _),
        Sysno::ftruncate => sys_ftruncate(tf.arg0() as _, tf.arg1() as _),
        Sysno::fallocate => sys_fallocate(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::fsync => sys_fsync(tf.arg0() as _),
        Sysno::fdatasync => sys_fdatasync(tf.arg0() as _),
        Sysno::pread64 => sys_pread64(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::pwrite64 => sys_pwrite64(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::preadv => sys_preadv(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::pwritev => sys_pwritev(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::preadv2 => sys_preadv2(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
        ),
        Sysno::pwritev2 => sys_pwritev2(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
        ),
        Sysno::sendfile => sys_sendfile(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::copy_file_range => sys_copy_file_range(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4() as _,
            tf.arg5() as _,
        ),
        Sysno::splice => sys_splice(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4() as _,
            tf.arg5() as _,
        ),

        // iomux
        #[cfg(target_arch = "x86_64")]
        Sysno::poll => sys_poll(tf.arg0().into(), tf.arg1() as _, tf.arg2() as _),
        Sysno::ppoll => sys_ppoll(
            tf.arg0().into(),
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3().into(),
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::select => sys_select(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3().into(),
            tf.arg4().into(),
        ),
        Sysno::pselect6 => sys_pselect6(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3().into(),
            tf.arg4().into(),
            tf.arg5().into(),
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::epoll_create => sys_epoll_create(tf.arg0() as _),
        Sysno::epoll_create1 => sys_epoll_create1(tf.arg0() as _),
        Sysno::epoll_ctl => sys_epoll_ctl(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3().into(),
        ),
        Sysno::epoll_pwait => sys_epoll_wait(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::epoll_pwait2 => sys_epoll_pwait2(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4().into(),
        ),

        // fs mount
        Sysno::mount => sys_mount(
            tf.arg0().into(),
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
            tf.arg4().into(),
        ) as _,
        Sysno::umount2 => sys_umount2(tf.arg0().into(), tf.arg1() as _) as _,

        // pipe
        Sysno::pipe2 => sys_pipe2(tf.arg0().into(), tf.arg1() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::pipe => sys_pipe2(tf.arg0().into(), 0),

        // fd
        Sysno::eventfd2 => sys_eventfd2(tf.arg0() as _, tf.arg1() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::eventfd => sys_eventfd(tf.arg0() as _),
        Sysno::timerfd_create => sys_timerfd_create(tf.arg0() as _, tf.arg1() as _),
        Sysno::timerfd_settime => sys_timerfd_settime(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3().into(),
        ),
        Sysno::timerfd_gettime => {
            sys_timerfd_gettime(tf.arg0() as _, tf.arg1().into(), tf.arg2().into())
        }
        Sysno::pidfd_open => sys_pidfd_open(tf.arg0() as _, tf.arg1() as _),
        Sysno::pidfd_getfd => sys_pidfd_getfd(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::pidfd_send_signal => sys_pidfd_send_signal(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3() as _,
        ),

        // fs stat
        #[cfg(target_arch = "x86_64")]
        Sysno::stat => sys_stat(tf.arg0().into(), tf.arg1().into()),
        Sysno::fstat => sys_fstat(tf.arg0() as _, tf.arg1().into()),
        #[cfg(target_arch = "x86_64")]
        Sysno::lstat => sys_lstat(tf.arg0().into(), tf.arg1().into()),
        #[cfg(target_arch = "x86_64")]
        Sysno::newfstatat => sys_fstatat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        #[cfg(not(target_arch = "x86_64"))]
        Sysno::fstatat => sys_fstatat(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::statx => sys_statx(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4().into(),
        ),
        #[cfg(target_arch = "x86_64")]
        Sysno::access => sys_access(tf.arg0().into(), tf.arg1() as _),
        Sysno::faccessat | Sysno::faccessat2 => sys_faccessat2(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::statfs => sys_statfs(tf.arg0().into(), tf.arg1().into()),
        Sysno::fstatfs => sys_fstatfs(tf.arg0() as _, tf.arg1().into()),

        // mm
        Sysno::brk => sys_brk(tf.arg0() as _),
        Sysno::mmap => sys_mmap(
            tf.arg0(),
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
            tf.arg5() as _,
        ),
        Sysno::munmap => sys_munmap(tf.arg0(), tf.arg1() as _),
        Sysno::mprotect => sys_mprotect(tf.arg0(), tf.arg1() as _, tf.arg2() as _),
        Sysno::msync => sys_msync(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::madvise => sys_madvise(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::mlock => Ok(0),
        Sysno::munlock => Ok(0),
        Sysno::membarrier => Ok(0),
        Sysno::mremap => Ok(0),

        // task info
        Sysno::getpid => sys_getpid(),
        Sysno::getppid => sys_getppid(),
        Sysno::gettid => sys_gettid(),

        // task sched
        Sysno::sched_yield => sys_sched_yield(),
        Sysno::sched_setaffinity => {
            sys_sched_setaffinity(tf.arg0() as _, tf.arg1() as _, tf.arg2().into())
        }
        Sysno::sched_getaffinity => {
            sys_sched_getaffinity(tf.arg0() as _, tf.arg1() as _, tf.arg2().into())
        }
        Sysno::sched_getparam => sys_sched_getparam(tf.arg0() as _, tf.arg1().into()),
        Sysno::sched_setparam => sys_sched_setparam(tf.arg0() as _, tf.arg1().into()),
        Sysno::sched_setscheduler => {
            sys_sched_setscheduler(tf.arg0() as _, tf.arg1() as _, tf.arg2().into())
        }
        Sysno::sched_getscheduler => sys_sched_getscheduler(tf.arg0() as _),
        Sysno::getpriority => sys_getpriority(tf.arg0() as _, tf.arg1() as _),
        Sysno::setpriority => sys_setpriority(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        // #[cfg(target_arch = "x86_64")]
        // Sysno::sched_get_priority_max => sys_sched_getscheduler_max(tf.arg0() as _, tf.arg1() as _),
        // #[cfg(target_arch = "x86_64")]
        // Sysno::sched_get_priority_min => sys_sched_getscheduler_min(tf.arg0() as _, tf.arg1() as _),
        Sysno::nanosleep => sys_nanosleep(tf.arg0().into(), tf.arg1().into()),
        Sysno::clock_nanosleep => sys_clock_nanosleep(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3().into(),
        ),

        // task ops
        Sysno::execve => sys_execve(tf, tf.arg0().into(), tf.arg1().into(), tf.arg2().into()),
        Sysno::set_tid_address => sys_set_tid_address(tf.arg0()),
        #[cfg(target_arch = "x86_64")]
        Sysno::arch_prctl => sys_arch_prctl(tf, tf.arg0() as _, tf.arg1() as _),
        Sysno::prlimit64 => sys_prlimit64(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3().into(),
        ),
        Sysno::capget => sys_capget(tf.arg0().into(), tf.arg1().into()),
        Sysno::capset => sys_capset(tf.arg0().into(), tf.arg1().into()),
        Sysno::prctl => Ok(0),

        // task management
        Sysno::clone => sys_clone(
            tf,
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2(),
            tf.arg3(),
            tf.arg4(),
        ),
        Sysno::clone3 => sys_clone3(tf, tf.arg0().into(), tf.arg1() as _),
        #[cfg(target_arch = "x86_64")]
        Sysno::fork => sys_fork(tf),
        Sysno::exit => sys_exit(tf.arg0() as _),
        Sysno::exit_group => sys_exit_group(tf.arg0() as _),
        Sysno::wait4 => sys_wait4(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::getsid => sys_getsid(tf.arg0() as _),
        Sysno::setsid => sys_setsid(),
        Sysno::getpgid => sys_getpgid(tf.arg0() as _),
        Sysno::setpgid => sys_setpgid(tf.arg0() as _, tf.arg1() as _),
        Sysno::waitid => Ok(0),

        // signal
        Sysno::rt_sigprocmask => sys_rt_sigprocmask(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::rt_sigaction => sys_rt_sigaction(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::rt_sigpending => sys_rt_sigpending(tf.arg0().into(), tf.arg1() as _),
        Sysno::rt_sigreturn => sys_rt_sigreturn(tf),
        Sysno::rt_sigtimedwait => sys_rt_sigtimedwait(
            tf.arg0().into(),
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::rt_sigsuspend => sys_rt_sigsuspend(tf, tf.arg0().into(), tf.arg1() as _),
        Sysno::kill => sys_kill(tf.arg0() as _, tf.arg1() as _),
        Sysno::tkill => sys_tkill(tf.arg0() as _, tf.arg1() as _),
        Sysno::tgkill => sys_tgkill(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::rt_sigqueueinfo => sys_rt_sigqueueinfo(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::rt_tgsigqueueinfo => sys_rt_tgsigqueueinfo(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4() as _,
        ),
        Sysno::sigaltstack => sys_sigaltstack(tf.arg0().into(), tf.arg1().into()),
        Sysno::futex => sys_futex(
            tf.arg0().into(),
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4().into(),
            tf.arg5() as _,
        ),
        Sysno::get_robust_list => {
            sys_get_robust_list(tf.arg0() as _, tf.arg1().into(), tf.arg2().into())
        }
        Sysno::set_robust_list => sys_set_robust_list(tf.arg0().into(), tf.arg1() as _),

        // cred
        Sysno::getuid => sys_getuid(),
        Sysno::setuid => sys_setuid(tf.arg0() as _),
        Sysno::getgid => sys_getgid(),
        Sysno::setgid => sys_setgid(tf.arg0() as _),
        Sysno::setfsuid => sys_setfsuid(tf.arg0() as _),
        Sysno::setfsgid => sys_setfsgid(tf.arg0() as _),
        Sysno::geteuid => sys_geteuid(),
        Sysno::getegid => sys_getegid(),
        Sysno::setreuid => sys_setreuid(tf.arg0() as _, tf.arg1() as _),
        Sysno::setregid => sys_setregid(tf.arg0() as _, tf.arg1() as _),
        Sysno::getresuid => sys_getresuid(tf.arg0().into(), tf.arg1().into(), tf.arg2().into()),
        Sysno::setresuid => sys_setresuid(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::getresgid => sys_getresgid(tf.arg0().into(), tf.arg1().into(), tf.arg2().into()),
        Sysno::setresgid => sys_setresgid(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::getgroups => sys_getgroups(tf.arg0() as _, tf.arg1().into()),
        Sysno::setgroups => sys_setgroups(tf.arg0() as _, tf.arg1().into()),

        // sys
        Sysno::getrandom => sys_getrandom(tf.arg0().into(), tf.arg1() as _, tf.arg2() as _),
        Sysno::getrusage => sys_getrusage(tf.arg0() as _, tf.arg1().into()),
        Sysno::uname => sys_uname(tf.arg0().into()),
        Sysno::sysinfo => sys_sysinfo(tf.arg0().into()),
        Sysno::syslog => sys_syslog(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::sethostname => Ok(0),
        Sysno::personality => Ok(0),
        Sysno::chroot => Err(LinuxError::EPERM),
        Sysno::reboot => Ok(0),
        Sysno::fanotify_init => sys_fanotify_init(tf.arg0() as _, tf.arg1() as _),
        Sysno::fanotify_mark => sys_fanotify_mark(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4().into(),
        ),
        Sysno::pkey_alloc => Ok(0),
        Sysno::pkey_free => Ok(0),
        Sysno::pkey_mprotect => Ok(0),
        Sysno::add_key => Ok(0),
        Sysno::keyctl => Ok(0),
        Sysno::request_key => Ok(0),
        Sysno::mincore => Ok(0),
        Sysno::ptrace => Ok(0),
        Sysno::mq_timedsend => Ok(0),
        Sysno::mq_timedreceive => Ok(0),
        Sysno::mq_notify => Ok(0),
        Sysno::mq_getsetattr => Ok(0),
        Sysno::mq_open => Ok(0),
        Sysno::mq_unlink => Ok(0),

        // time
        Sysno::gettimeofday => sys_gettimeofday(tf.arg0().into()),
        Sysno::getitimer => sys_getitimer(tf.arg0() as _, tf.arg1().into()),
        Sysno::setitimer => sys_setitimer(tf.arg0() as _, tf.arg1().into(), tf.arg2().into()),
        Sysno::times => sys_times(tf.arg0().into()),
        Sysno::timer_create => sys_timer_create(tf.arg0() as _, tf.arg1().into(), tf.arg2().into()),
        Sysno::clock_gettime => sys_clock_gettime(tf.arg0() as _, tf.arg1().into()),
        Sysno::clock_settime => sys_clock_settime(tf.arg0() as _, tf.arg1().into()),
        Sysno::clock_getres => sys_clock_getres(tf.arg0() as _, tf.arg1().into()),
        Sysno::clock_adjtime => Ok(0),
        Sysno::adjtimex => Err(LinuxError::EINVAL),

        // shm
        Sysno::shmget => sys_shmget(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::shmat => sys_shmat(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::shmctl => sys_shmctl(tf.arg0() as _, tf.arg1() as _, tf.arg2().into()),
        Sysno::shmdt => sys_shmdt(tf.arg0() as _),

        // msg
        Sysno::msgget => sys_msgget(tf.arg0() as _, tf.arg1() as _),
        Sysno::msgctl => sys_msgctl(tf.arg0() as _, tf.arg1() as _, tf.arg2().into()),
        Sysno::msgsnd => sys_msgsnd(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::msgrcv => sys_msgrcv(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
        ),

        // sem
        Sysno::semget => sys_semget(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::semctl => sys_semctl(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
        ),
        Sysno::semop => sys_semop(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),

        // net
        Sysno::socket => sys_socket(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        Sysno::bind => sys_bind(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::connect => sys_connect(tf.arg0() as _, tf.arg1().into(), tf.arg2() as _),
        Sysno::getsockname => sys_getsockname(tf.arg0() as _, tf.arg1().into(), tf.arg2().into()),
        Sysno::getpeername => sys_getpeername(tf.arg0() as _, tf.arg1().into(), tf.arg2().into()),
        Sysno::setsockopt => sys_setsockopt(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4() as _,
        ),
        Sysno::getsockopt => sys_getsockopt(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3().into(),
            tf.arg4().into(),
        ),
        Sysno::listen => sys_listen(tf.arg0() as _, tf.arg1() as _),
        Sysno::accept => sys_accept(tf.arg0() as _, tf.arg1().into(), tf.arg2().into()),
        Sysno::accept4 => sys_accept4(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2().into(),
            tf.arg3() as _,
        ),
        Sysno::sendto => sys_sendto(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4().into(),
            tf.arg5() as _,
        ),
        Sysno::recvfrom => sys_recvfrom(
            tf.arg0() as _,
            tf.arg1().into(),
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4().into(),
            tf.arg5().into(),
        ),
        Sysno::socketpair => sys_socketpair(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3().into(),
        ),
        Sysno::shutdown => sys_shutdown(tf.arg0() as _, tf.arg1() as _),

        Sysno::signalfd4
        | Sysno::inotify_init1
        | Sysno::userfaultfd
        | Sysno::perf_event_open
        | Sysno::io_uring_setup
        | Sysno::bpf
        | Sysno::fsopen
        | Sysno::fspick
        | Sysno::open_tree
        | Sysno::memfd_create
        | Sysno::memfd_secret => sys_dummy_fd(),
        _ => {
            warn!("Unimplemented syscall: {}", sysno);
            Err(LinuxError::ENOSYS)
        }
    }
}

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &mut TrapFrame, syscall_num: usize) -> isize {
    let sysno = Sysno::new(syscall_num);
    trace!("Syscall {:?}", sysno);

    time_stat_from_user_to_kernel();

    let result = sysno
        .ok_or(LinuxError::ENOSYS)
        .and_then(|sysno| handle_syscall_impl(tf, sysno));
    debug!(
        "Syscall {} return {:?}",
        sysno.map_or("(invalid)".to_string(), |s| s.to_string()),
        result
    );

    time_stat_from_kernel_to_user();
    result.unwrap_or_else(|err| -err.code() as _)
}
