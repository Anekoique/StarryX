use axhal::{
    arch::TrapFrame,
    trap::{POST_TRAP, register_trap_handler},
};
use axsignal::{SignalOSAction, SignalSet};
use axtask::{TaskExtRef, current};

use crate::do_exit;

pub fn check_signals(tf: &mut TrapFrame, restore_blocked: Option<SignalSet>) -> bool {
    let Some((sig, os_action)) = current()
        .task_ext()
        .thread_data()
        .signal
        .check_signals(tf, restore_blocked)
    else {
        return false;
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
            // TODO: implement stop
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
    let Some((sig, os_action)) = current()
        .task_ext()
        .thread_data()
        .signal
        .check_fatal_signals()
    else {
        return;
    };

    let signo = sig.signo();
    match os_action {
        SignalOSAction::Terminate => {
            do_exit(128 + signo as i32, true);
        }
        SignalOSAction::Stop => {
            // TODO: implement stop
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
