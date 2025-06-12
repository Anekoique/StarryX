use axerrno::LinuxResult;
use axsignal::SignalSet;
use alloc::vec::Vec;

use crate::{
    ctypes::{timespec, timeval},
    fs::FD_TABLE,
    ptr::{UserConstPtr, UserPtr},
    time::{TimeValue, timespec_to_timevalue, timeval_to_timevalue, wall_time},
};

// Helper function to debug fd_set parsing
fn debug_fdset(name: &str, fds: &Option<Vec<u8>>, nfds: u32) {
    if let Some(fds) = fds {
        debug!("{}: bytes = {:?}", name, fds);
        let mut set_fds = Vec::new();
        for fd in 0..(nfds as usize) {
            let byte_index = fd / 8;
            let bit_index = fd % 8;
            if byte_index < fds.len() && (fds[byte_index] & (1 << bit_index)) != 0 {
                set_fds.push(fd);
            }
        }
        if !set_fds.is_empty() {
            debug!("{}: set fds = {:?}", name, set_fds);
        } else {
            debug!("{}: no fds set", name);
        }
    } else {
        debug!("{}: null", name);
    }
}

fn do_select(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: Option<TimeValue>,
) -> LinuxResult<isize> {
    let num_bytes = nfds.div_ceil(8) as usize;
    
    // First read the current fd_sets to see which fds the user wants to monitor
    let mut read_fdset = if !read_fds.is_null() {
        Some(read_fds.get_as_mut_slice(num_bytes)?)
    } else {
        None
    };
    let mut write_fdset = if !write_fds.is_null() {
        Some(write_fds.get_as_mut_slice(num_bytes)?)
    } else {
        None
    };
    let mut except_fdset = if !except_fds.is_null() {
        Some(except_fds.get_as_mut_slice(num_bytes)?)
    } else {
        None
    };

    // Store the original fd_sets to know which fds to check
    let orig_read_fds = read_fdset.as_ref().map(|fds| fds.to_vec());
    let orig_write_fds = write_fdset.as_ref().map(|fds| fds.to_vec());
    let orig_except_fds = except_fdset.as_ref().map(|fds| fds.to_vec());

    // Debug the original fd_sets with detailed analysis
    debug_fdset("original read_fds", &orig_read_fds, nfds);
    debug_fdset("original write_fds", &orig_write_fds, nfds);
    debug_fdset("original except_fds", &orig_except_fds, nfds);

    // Helper function to check if fd is set in fd_set
    fn is_fd_set(fds: &Option<Vec<u8>>, fd: usize) -> bool {
        if let Some(fds) = fds {
            let byte_index = fd / 8;
            let bit_index = fd % 8;
            if byte_index < fds.len() {
                (fds[byte_index] & (1 << bit_index)) != 0
            } else {
                false
            }
        } else {
            false
        }
    }

    // Helper function to set fd in fd_set
    fn set_fd(fds: &mut Option<&mut [u8]>, fd: usize) -> bool {
        if let Some(fds) = fds {
            let byte_index = fd / 8;
            let bit_index = fd % 8;
            if byte_index < fds.len() {
                fds[byte_index] |= 1 << bit_index;
                debug!("set_fd: fd {} set in result (byte_index={}, bit_index={})", 
                       fd, byte_index, bit_index);
                return true;
            }
        }
        false
    }

    // Validate all requested file descriptors before starting the polling loop
    let fd_table = FD_TABLE.read();
    let mut valid_fds = 0;
    for fd in 0..(nfds as usize) {
        if is_fd_set(&orig_read_fds, fd) ||
           is_fd_set(&orig_write_fds, fd) ||
           is_fd_set(&orig_except_fds, fd) {
            if fd_table.get(fd).is_none() {
                debug!("select: fd {} is set but not found in fd_table, returning EBADF", fd);
                return Err(axerrno::LinuxError::EBADF);
            } else {
                valid_fds += 1;
                debug!("select: fd {} validated successfully", fd);
            }
        }
    }
    debug!("select: validated {} file descriptors", valid_fds);
    drop(fd_table);

    let deadline = timeout.map(|t| wall_time() + t);

    debug!(
        "select timeout: {:?} nfds={} read={} write={} except={}",
        timeout,
        nfds,
        read_fdset.is_some(),
        write_fdset.is_some(),
        except_fdset.is_some()
    );

    // Log which specific fds we're monitoring
    let mut monitored_fds = Vec::new();
    for fd in 0..(nfds as usize) {
        if is_fd_set(&orig_read_fds, fd) ||
           is_fd_set(&orig_write_fds, fd) ||
           is_fd_set(&orig_except_fds, fd) {
            monitored_fds.push(fd);
        }
    }
    debug!("select: monitoring {} file descriptors: {:?}", monitored_fds.len(), monitored_fds);

    // 改进的轮询策略
    let mut poll_count = 0;
    const IMMEDIATE_POLLS: u32 = 3;                    // 增加立即轮询次数
    const FAST_POLL_MICROS: u64 = 100;                // 快速轮询间隔：100μs
    const MEDIUM_POLL_MICROS: u64 = 1_000;            // 中等轮询间隔：1ms
    const SLOW_POLL_MICROS: u64 = 5_000;              // 慢速轮询间隔：5ms
    const MAX_POLL_MICROS: u64 = 10_000;              // 最大轮询间隔：10ms
    const FAST_POLL_THRESHOLD: u32 = 10;              // 快速轮询阈值
    const MEDIUM_POLL_THRESHOLD: u32 = 50;            // 中等轮询阈值
    
    loop {
        // Clear all fd_sets first
        if let Some(fds) = read_fdset.as_mut() {
            fds.fill(0);
        }
        if let Some(fds) = write_fdset.as_mut() {
            fds.fill(0);
        }
        if let Some(fds) = except_fdset.as_mut() {
            fds.fill(0);
        }

        let mut total_ready = 0;
        
        // 在网络 I/O 密集型场景下，确保网络接口得到处理
        // 这可以帮助减少网络延迟和提高响应性
        if poll_count > 0 && poll_count % 5 == 0 {
            // 每5次轮询主动触发一次网络处理
            // 注意：这里假设存在网络处理函数，具体实现需要根据系统架构调整
            axtask::yield_now();
        }
        
        let fd_table = FD_TABLE.read();

        // Only check file descriptors that were originally set and are within nfds
        for fd in 0..(nfds as usize) {
            let should_check_read = is_fd_set(&orig_read_fds, fd);
            let should_check_write = is_fd_set(&orig_write_fds, fd);
            let should_check_except = is_fd_set(&orig_except_fds, fd);

            if !should_check_read && !should_check_write && !should_check_except {
                continue;
            }

            // Get the file descriptor if it exists
            if let Some(file) = fd_table.get(fd) {
                debug!("select: checking fd {} (read={}, write={}, except={})", 
                       fd, should_check_read, should_check_write, should_check_except);
                
                match file.poll() {
                    Ok(state) => {
                        debug!("select: fd {} poll state - readable={}, writable={}", 
                               fd, state.readable, state.writable);
                        
                        if should_check_read && state.readable {
                            if set_fd(&mut read_fdset, fd) {
                                debug!("select: fd {} marked as readable (poll success)", fd);
                                total_ready += 1;
                            }
                        }
                        if should_check_write && state.writable {
                            if set_fd(&mut write_fdset, fd) {
                                debug!("select: fd {} marked as writable (poll success)", fd);
                                total_ready += 1;
                            }
                        }
                        if should_check_except {
                            // TODO: Implement exception checking when PollState supports it
                            // For now, exceptions are only triggered by poll errors
                        }
                    }
                    Err(e) => {
                        debug!("select: fd {} poll failed: {:?}", fd, e);
                        // If polling fails, treat as an exception
                        if should_check_except {
                            if set_fd(&mut except_fdset, fd) {
                                debug!("select: fd {} marked as exception due to poll error", fd);
                                total_ready += 1;
                            }
                        }
                    }
                }
            } else {
                // This should not happen as we validated all fds earlier
                debug!("select: fd {} not found in fd_table (unexpected)", fd);
                return Err(axerrno::LinuxError::EBADF);
            }
        }

        drop(fd_table);

        if total_ready > 0 {
            debug!("select poll result: total={}, poll_count={}", total_ready, poll_count);
            
            // Log detailed results
            for fd in 0..(nfds as usize) {
                if is_fd_set(&read_fdset.as_ref().map(|fds| fds.to_vec()), fd) {
                    debug!("select result: fd {} ready for READ", fd);
                }
                if is_fd_set(&write_fdset.as_ref().map(|fds| fds.to_vec()), fd) {
                    debug!("select result: fd {} ready for WRITE", fd);
                }
                if is_fd_set(&except_fdset.as_ref().map(|fds| fds.to_vec()), fd) {
                    debug!("select result: fd {} has EXCEPTION", fd);
                }
            }
            
            return Ok(total_ready as isize);
        }

        // Check timeout before sleeping
        if let Some(deadline) = deadline {
            let current_time = wall_time();
            if current_time >= deadline {
                debug!("select: timeout reached after {} polls, returning 0", poll_count);
                return Ok(0);
            }
            
            // 如果剩余时间很少，再做一次快速检查
            let remaining = deadline - current_time;
            if remaining.as_micros() < 500 { // 小于500μs
                debug!("select: remaining time < 500μs, doing final check");
                continue; // 再检查一次，然后超时
            }
        }

        poll_count += 1;

        // 改进的睡眠策略：更智能的退避算法
        if poll_count <= IMMEDIATE_POLLS {
            // 立即重试，不睡眠，但让出CPU
            axtask::yield_now();
        } else {
            // 分阶段的睡眠策略
            let sleep_micros = if poll_count <= IMMEDIATE_POLLS + FAST_POLL_THRESHOLD {
                // 快速轮询阶段：适合网络I/O的快速响应
                FAST_POLL_MICROS
            } else if poll_count <= IMMEDIATE_POLLS + MEDIUM_POLL_THRESHOLD {
                // 中等轮询阶段：平衡响应性和CPU使用
                MEDIUM_POLL_MICROS
            } else {
                // 慢速轮询阶段：减少CPU占用
                let backoff_steps = poll_count - IMMEDIATE_POLLS - MEDIUM_POLL_THRESHOLD;
                let sleep_time = SLOW_POLL_MICROS + (backoff_steps as u64) * 1000; // 每步增加1ms
                sleep_time.min(MAX_POLL_MICROS)
            };
            
            let sleep_interval = TimeValue::from_micros(sleep_micros);
            
            // 如果有超时，确保睡眠时间不超过剩余时间
            let actual_sleep = if let Some(deadline) = deadline {
                let remaining = deadline.saturating_sub(wall_time());
                if remaining < sleep_interval {
                    remaining
                } else {
                    sleep_interval
                }
            } else {
                sleep_interval
            };
            
            if actual_sleep.as_micros() > 0 {
                axtask::sleep(actual_sleep);
            }
        }

        // 改进的防护机制：更早检测异常情况
        if poll_count > 1000 { // 降低阈值，更早检测问题
            warn!("select: excessive polling detected ({}), returning timeout", poll_count);
            return Ok(0);
        }
    }
}

pub fn sys_select(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: UserConstPtr<timeval>,
) -> LinuxResult<isize> {
    let timeout_val = if !timeout.is_null() {
        Some(timeval_to_timevalue(*timeout.get_as_ref()?))
    } else {
        None
    };
    
    do_select(
        nfds,
        read_fds,
        write_fds,
        except_fds,
        timeout_val,
    )
}

pub fn sys_pselect6(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: UserConstPtr<timespec>,
    _sigmask: UserConstPtr<SignalSet>,
) -> LinuxResult<isize> {
    let timeout_val = if !timeout.is_null() {
        Some(timespec_to_timevalue(*timeout.get_as_ref()?))
    } else {
        None
    };
    
    do_select(
        nfds,
        read_fds,
        write_fds,
        except_fds,
        timeout_val,
    )
}