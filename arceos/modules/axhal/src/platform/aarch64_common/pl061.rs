//! PrimeCell GPIO (PL061) Driver for QEMU ARM64 Virtual Platform
//!
//! This module implements a GPIO interrupt handler for graceful system shutdown
//! on QEMU ARM64 virt platform. It provides hardware abstraction for the PL061 
//! GPIO controller, specifically configured to handle shutdown signals.
//!
//! ## Features
//! - GPIO pin 3 interrupt handling for system shutdown
//! - Level-sensitive interrupt configuration
//! - Integration with QEMU monitor `system_powerdown` command
//! - ARM64 system shutdown via HLT instruction
//!
//! ## Hardware Configuration
//! - Uses GPIO pin 3 as shutdown signal input
//! - Configured for high-level sensitive interrupts
//! - Mapped to IRQ 39 in the system interrupt controller

use crate::mem::phys_to_virt;
use axconfig::devices::{GPIO_IRQ, GPIO_PADDR};
use kspin::SpinNoIrq;
use memory_addr::PhysAddr;

// =============================================================================
// PL061 GPIO Register Definitions
// =============================================================================

/// GPIO data direction register - controls input/output mode for each pin
const GPIO_DIR: usize = 0x400;

/// GPIO interrupt sense register - configures edge/level sensitivity
const GPIO_IS: usize = 0x404;

/// GPIO interrupt both edges register - enables both edge detection
const GPIO_IBE: usize = 0x408;

/// GPIO interrupt event register - configures high/low level or rising/falling edge
const GPIO_IEV: usize = 0x40C;

/// GPIO interrupt mask register - enables/disables interrupts for each pin
const GPIO_IE: usize = 0x410;

/// GPIO raw interrupt status register - shows unmasked interrupt status
const GPIO_RIS: usize = 0x414;

/// GPIO interrupt clear register - clears pending interrupts
const GPIO_IC: usize = 0x41C;

// =============================================================================
// GPIO Pin Configuration
// =============================================================================

/// Bit mask for GPIO pin 3 - used for shutdown signal detection
/// This pin is connected to QEMU's system powerdown functionality
const GPIO_PIN3_MASK: u32 = 1 << 3;

// =============================================================================
// Driver Implementation
// =============================================================================

/// Internal GPIO driver state protected by spinlock
struct GPIOInner {
    /// Virtual base address of GPIO register space
    base_addr: usize,
    /// Initialization flag to prevent double initialization
    initialized: bool,
}

/// PL061 GPIO driver for handling system shutdown signals
/// 
/// This driver provides a high-level interface to the PL061 GPIO controller,
/// specifically configured for shutdown signal detection on QEMU ARM64 virt platform.
pub struct GPIO {
    /// Thread-safe inner state protected by spinlock
    inner: SpinNoIrq<GPIOInner>,
}

impl GPIOInner {
    /// Create a new uninitialized GPIO driver instance
    const fn new() -> Self {
        Self {
            base_addr: 0,
            initialized: false,
        }
    }

    /// Read a 32-bit value from GPIO register
    /// 
    /// # Arguments
    /// * `offset` - Register offset from base address
    /// 
    /// # Returns
    /// The 32-bit register value
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((self.base_addr + offset) as *const u32) }
    }

    /// Write a 32-bit value to GPIO register
    /// 
    /// # Arguments
    /// * `offset` - Register offset from base address
    /// * `value` - 32-bit value to write
    fn write_reg(&self, offset: usize, value: u32) {
        unsafe { core::ptr::write_volatile((self.base_addr + offset) as *mut u32, value) }
    }

    /// Initialize GPIO hardware for shutdown signal detection
    /// 
    /// This function configures GPIO pin 3 as an input with level-sensitive
    /// interrupt capability. The configuration enables detection of QEMU's
    /// system powerdown signal.
    /// 
    /// # Configuration Steps
    /// 1. Map physical GPIO address to virtual address space
    /// 2. Clear any pending interrupts from previous operations
    /// 3. Configure pin 3 as input
    /// 4. Set level-sensitive interrupt mode
    /// 5. Configure for high-level trigger
    /// 6. Enable interrupt for pin 3
    fn init(&mut self) {
        // Prevent double initialization
        if self.initialized {
            return;
        }
        
        info!("Initializing PL061 GPIO for shutdown detection...");
        
        // Map GPIO physical address to virtual address space
        self.base_addr = phys_to_virt(PhysAddr::from(GPIO_PADDR)).as_usize();
        self.initialized = true;
        
        // Clear all pending interrupts before configuration
        self.write_reg(GPIO_IC, 0xFF);
        
        // Configure GPIO pin 3 as input (clear bit 3 in direction register)
        let mut dir = self.read_reg(GPIO_DIR);
        dir &= !GPIO_PIN3_MASK;  // 0 = input, 1 = output
        self.write_reg(GPIO_DIR, dir);

        // Configure interrupt as level-sensitive (set bit 3)
        let mut is = self.read_reg(GPIO_IS);
        is |= GPIO_PIN3_MASK;  // 1 = level sensitive, 0 = edge sensitive
        self.write_reg(GPIO_IS, is);

        // Disable both edge detection for level-sensitive mode
        let mut ibe = self.read_reg(GPIO_IBE);
        ibe &= !GPIO_PIN3_MASK;  // 0 = use GPIOIEV, 1 = both edges
        self.write_reg(GPIO_IBE, ibe);

        // Configure for high level trigger (set bit 3)
        let mut iev = self.read_reg(GPIO_IEV);
        iev |= GPIO_PIN3_MASK;  // 1 = high level/rising edge, 0 = low level/falling edge
        self.write_reg(GPIO_IEV, iev);

        // Clear any pending interrupts after configuration
        self.write_reg(GPIO_IC, GPIO_PIN3_MASK);
        
        // Enable interrupt for pin 3 (set bit 3)
        let mut ie = self.read_reg(GPIO_IE);
        ie |= GPIO_PIN3_MASK;  // 1 = interrupt enabled, 0 = interrupt disabled
        self.write_reg(GPIO_IE, ie);

        info!("GPIO initialized for shutdown detection on pin 3");
    }

    /// Handle GPIO interrupt - processes shutdown signal
    /// 
    /// This function is called when a GPIO interrupt occurs. It checks if the
    /// interrupt is from pin 3 (shutdown signal) and initiates system shutdown.
    fn handle_irq(&mut self) {
        if !self.initialized {
            warn!("GPIO interrupt received but driver not initialized");
            return;
        }
        
        // Read raw interrupt status to check which pin triggered
        let ris = self.read_reg(GPIO_RIS);
        
        // Check if interrupt is from pin 3 (shutdown signal)
        if (ris & GPIO_PIN3_MASK) != 0 {
            info!("GPIO shutdown signal received!");
            
            // Clear the interrupt to prevent repeated triggers
            self.write_reg(GPIO_IC, GPIO_PIN3_MASK);
            
            // Initiate system shutdown sequence
            self.shutdown();
        }
    }

    /// Perform ARM64 system shutdown
    /// 
    /// This function executes the ARM64 system shutdown sequence using
    /// the HLT (halt) instruction with QEMU-specific parameters.
    /// 
    /// # Shutdown Sequence
    /// 1. Load shutdown code (0x18) into w0 register
    /// 2. Execute HLT instruction with QEMU poweroff code (0xF000)
    /// 
    /// # Note
    /// This function does not return as it halts the system.
    fn shutdown(&self) {
        info!("Performing system shutdown...");
        
        // Execute ARM64 shutdown sequence for QEMU
        // mov w0, #0x18   - Load QEMU shutdown code
        // hlt #0xF000     - Halt with QEMU poweroff parameter
        unsafe {
            core::arch::asm!(
                "mov w0, #0x18",
                "hlt #0xF000",
                options(noreturn)
            );
        }

        // Alternative: Use PSCI system_off function for ACPI-compliant shutdown
        // crate::platform::aarch64_common::psci::system_off();
    }
}

impl GPIO {
    /// Create a new GPIO driver instance
    const fn new() -> Self {
        Self {
            inner: SpinNoIrq::new(GPIOInner::new()),
        }
    }

    /// Initialize GPIO hardware (thread-safe wrapper)
    fn init(&self) {
        self.inner.lock().init();
    }

    /// Handle GPIO interrupt (thread-safe wrapper)
    fn handle_irq(&self) {
        self.inner.lock().handle_irq();
    }
}

/// Global GPIO driver instance
static GPIO_DRIVER: GPIO = GPIO::new();

/// GPIO interrupt service routine
/// 
/// This function is registered as the interrupt handler for GPIO IRQ.
/// It delegates to the GPIO driver's interrupt handling logic.
fn gpio_irq_handler() {
    GPIO_DRIVER.handle_irq();
}

/// Initialize GPIO driver and register interrupt handler
/// 
/// This function performs the complete GPIO driver initialization:
/// 1. Initialize GPIO hardware configuration
/// 2. Register interrupt handler in the system
/// 3. Enable GPIO IRQ in the interrupt controller
/// 
/// # Returns
/// This function logs success/failure but does not return error codes.
/// Initialization failures are logged as errors.
pub fn init() {
    info!("Initializing GPIO driver...");
    
    // Initialize GPIO hardware configuration
    GPIO_DRIVER.init();
    
    // Register GPIO interrupt handler with the kernel IRQ subsystem
    if crate::irq::register_handler(GPIO_IRQ, gpio_irq_handler) {
        info!("GPIO IRQ handler registered successfully for IRQ {}", GPIO_IRQ);
    } else {
        error!("Failed to register GPIO IRQ handler for IRQ {}", GPIO_IRQ);
    }
    
    // Enable GPIO IRQ in the Generic Interrupt Controller (GIC)
    crate::platform::irq::set_enable(GPIO_IRQ, true);
    
    info!("GPIO driver initialization completed");
}