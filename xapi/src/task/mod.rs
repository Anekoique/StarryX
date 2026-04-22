mod clone;
mod cred;
mod ctl;
mod execve;
mod exit;
mod futex;
mod schedule;
mod signal;
mod thread;
mod wait;

pub use self::clone::*;
pub use self::cred::*;
pub use self::ctl::*;
pub use self::execve::*;
pub use self::exit::*;
pub use self::futex::*;
pub use self::schedule::*;
pub use self::signal::*;
pub use self::thread::*;
pub use self::wait::*;

use axhal::{
    arch::TrapFrame,
    trap::{POST_TRAP, register_trap_handler},
};

use xsignal::{SignalOSAction, SignalSet, Signo};

use xcore::task::{XThread, with_current, with_thread, with_xthread};

pub fn check_signals(tf: &mut TrapFrame, restore_blocked: Option<SignalSet>) -> bool {
    let Some((sig, os_action)) = with_thread(|thread| {
        XThread::from_thread(thread)
            .signal
            .check_signals(tf, restore_blocked)
    }) else {
        return false;
    };

    debug!("handle signal: {:?}", sig.signo());
    let signo = sig.signo();
    if signo == Signo::SIGALRM {
        with_current(|curr| curr.set_interrupted(false));
    }
    match os_action {
        SignalOSAction::Terminate => {
            do_exit(128 + signo as i32, true);
        }
        SignalOSAction::CoreDump => {
            // TODO: implement core dump
            do_exit(128 + signo as i32, true);
        }
        SignalOSAction::Stop => {
            // TODO: implement proper process stopping
            // For now, ignore SIGTTIN/SIGTTOU to allow bash job control to work
            // without implementing full process suspension
            if matches!(signo, Signo::SIGTTIN | Signo::SIGTTOU) {
                debug!(
                    "Ignoring {:?} signal for temporary job control compatibility",
                    signo
                );
                return true; // Signal handled, don't kill the process
            }
            do_exit(1, true);
        }
        SignalOSAction::Continue => {
            // TODO: implement continue
        }
        SignalOSAction::Handler => {
            // do nothing
        }
    }
    true
}

pub fn check_fatal_signals() {
    let Some((sig, os_action)) = with_xthread(|xthread| xthread.signal.check_fatal_signals())
    else {
        return;
    };

    let signo = sig.signo();
    match os_action {
        SignalOSAction::Terminate => {
            do_exit(128 + signo as i32, true);
        }
        SignalOSAction::CoreDump => {
            // TODO: implement core dump
            do_exit(128 + signo as i32, true);
        }
        SignalOSAction::Stop => {
            do_exit(1, true);
        }
        _ => unreachable!("Only SIGKILL and SIGSTOP should be in fatal_signals"),
    }
}

#[register_trap_handler(POST_TRAP)]
fn post_trap_callback(tf: &mut TrapFrame, from_user: bool) {
    if !from_user {
        return;
    }

    check_signals(tf, None);
}
