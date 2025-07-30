//! Interrupt counting module for StarryX kernel
//! 
//! This module provides functionality to track interrupt counts for different
//! interrupt sources, specifically external interrupts (virtio storage devices)
//! and timer interrupts.

#![no_std]

use core::sync::atomic::{AtomicU64, Ordering};
use spin::RwLock;
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use alloc::format;

extern crate alloc;

/// Maximum number of interrupt lines we can track
#[warn(dead_code)]
const MAX_INTERRUPTS: usize = 1024;

/// Global interrupt counter storage
static INTERRUPT_COUNTERS: RwLock<BTreeMap<usize, AtomicU64>> = RwLock::new(BTreeMap::new());

/// Timer interrupt numbers for different architectures
#[cfg(target_arch = "riscv64")]
const TIMER_IRQ_NUM: usize = (1 << (usize::BITS - 1)) + 5; // S_TIMER from RISC-V

#[cfg(target_arch = "loongarch64")]
const TIMER_IRQ_NUM: usize = 11; // Timer interrupt for LoongArch64

/// Initialize the interrupt counting system
pub fn init() {
    // Initialize with empty counter map
    let mut counters = INTERRUPT_COUNTERS.write();
    *counters = BTreeMap::new();
}

/// Increment the counter for a specific interrupt number
pub fn increment_interrupt_count(irq_num: usize) {
    let counters = INTERRUPT_COUNTERS.read();
    
    // Check if this interrupt number already exists
    if let Some(counter) = counters.get(&irq_num) {
        counter.fetch_add(1, Ordering::Relaxed);
    } else {
        // Need to add new counter - drop read lock and acquire write lock
        drop(counters);
        let mut counters = INTERRUPT_COUNTERS.write();
        
        // Double-check in case another thread added it
        if let Some(counter) = counters.get(&irq_num) {
            counter.fetch_add(1, Ordering::Relaxed);
        } else {
            // Add new counter and increment it
            let counter = AtomicU64::new(1);
            counters.insert(irq_num, counter);
        }
    }
}

/// Get the current interrupt counts as a formatted string for /proc/interrupts
pub fn get_interrupt_counts() -> String {
    let counters = INTERRUPT_COUNTERS.read();
    let mut result = String::new();
    
    // Collect and sort interrupt numbers
    let mut irq_nums: Vec<usize> = counters.keys().cloned().collect();
    irq_nums.sort();
    
    // Handle timer interrupt priority - if external interrupt conflicts with timer,
    // only keep timer interrupt count
    let mut processed_irqs = BTreeMap::new();
    
    for &irq_num in &irq_nums {
        let count = counters.get(&irq_num).unwrap().load(Ordering::Relaxed);
        
        // Check if this is a timer interrupt
        if is_timer_interrupt(irq_num) {
            // Timer interrupt takes priority - remove any conflicting external interrupt
            processed_irqs.insert(irq_num, count);
        } else {
            // Only add external interrupt if no timer interrupt with same number exists
            if !processed_irqs.contains_key(&irq_num) {
                processed_irqs.insert(irq_num, count);
            }
        }
    }
    
    // Format output
    for (irq_num, count) in processed_irqs {
        result.push_str(&format!("{}:        {}\n", irq_num, count));
    }
    
    result
}

/// Check if an interrupt number corresponds to a timer interrupt
fn is_timer_interrupt(irq_num: usize) -> bool {
    irq_num == TIMER_IRQ_NUM
}

/// Get interrupt count for a specific IRQ number
pub fn get_interrupt_count(irq_num: usize) -> u64 {
    let counters = INTERRUPT_COUNTERS.read();
    counters.get(&irq_num)
        .map(|counter| counter.load(Ordering::Relaxed))
        .unwrap_or(0)
}

/// Reset all interrupt counters (for testing purposes)
#[cfg(test)]
pub fn reset_counters() {
    let mut counters = INTERRUPT_COUNTERS.write();
    counters.clear();
}
