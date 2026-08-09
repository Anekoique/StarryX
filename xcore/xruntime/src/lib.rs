//! Runtime library of [ArceOS](https://github.com/arceos-org/arceos).
//!
//! Any application uses ArceOS should link this library. It does some
//! initialization work before entering the application's `main` function.
//!
//! # Cargo Features
//!
//! - `alloc`: Enable global memory allocator.
//! - `paging`: Enable page table manipulation support.
//! - `irq`: Enable interrupt handling support.
//! - `multitask`: Enable multi-threading support.
//! - `smp`: Enable SMP (symmetric multiprocessing) support.
//! - `fs`: Enable filesystem support.
//! - `net`: Enable networking support.
//! - `display`: Enable graphics support.
//!
//! All the features are optional and disabled by default.

#![cfg_attr(not(test), no_std)]
#[macro_use]
extern crate xlog;

#[cfg(all(target_os = "none", not(test)))]
mod lang_items;

#[cfg(feature = "smp")]
mod mp;

#[cfg(feature = "smp")]
pub use self::mp::rust_main_secondary;

// const LOGO: &str = r#"
//        d8888                            .d88888b.   .d8888b.
//       d88888                           d88P" "Y88b d88P  Y88b
//      d88P888                           888     888 Y88b.
//     d88P 888 888d888  .d8888b  .d88b.  888     888  "Y888b.
//    d88P  888 888P"   d88P"    d8P  Y8b 888     888     "Y88b.
//   d88P   888 888     888      88888888 888     888       "888
//  d8888888888 888     Y88b.    Y8b.     Y88b. .d88P Y88b  d88P
// d88P     888 888      "Y8888P  "Y8888   "Y88888P"   "Y8888P"
// "#;

unsafe extern "C" {
    fn main();
}

/// Hook called from the timer IRQ to refresh the vDSO data page.
///
/// The `xvdso` component provides the impl. If a downstream build doesn't
/// include that component (e.g. an embedded ArceOS app), it must
/// supply its own no-op `impl VdsoTickIf for ...`.
#[crate_interface::def_interface]
pub trait VdsoTickIf {
    fn on_timer_tick();
}

struct LogIfImpl;

#[crate_interface::impl_interface]
impl xlog::LogIf for LogIfImpl {
    fn console_write_str(s: &str) {
        xhal::console::write_bytes(s.as_bytes());
    }

    fn current_time() -> core::time::Duration {
        xhal::time::monotonic_time()
    }

    fn current_cpu_id() -> Option<usize> {
        #[cfg(feature = "smp")]
        if is_init_ok() {
            Some(xhal::cpu::this_cpu_id())
        } else {
            None
        }
        #[cfg(not(feature = "smp"))]
        Some(0)
    }

    fn current_task_id() -> Option<u64> {
        if is_init_ok() {
            #[cfg(feature = "multitask")]
            {
                xtask::current_may_uninit().map(|curr| curr.id().as_u64())
            }
            #[cfg(not(feature = "multitask"))]
            None
        } else {
            None
        }
    }
}

use core::sync::atomic::{AtomicUsize, Ordering};

static INITED_CPUS: AtomicUsize = AtomicUsize::new(0);

fn is_init_ok() -> bool {
    if xconfig::PLATFORM == "riscv64-visionfive2" {
        return true;
    }
    INITED_CPUS.load(Ordering::Acquire) == xconfig::SMP
}

/// The main entry point of the ArceOS runtime.
///
/// It is called from the bootstrapping code in [xhal]. `cpu_id` is the ID of
/// the current CPU, and `dtb` is the address of the device tree blob. It
/// finally calls the application's `main` function after all initialization
/// work is done.
///
/// In multi-core environment, this function is called on the primary CPU,
/// and the secondary CPUs call [`rust_main_secondary`].
#[cfg_attr(not(test), unsafe(no_mangle))]
pub extern "C" fn rust_main(cpu_id: usize, dtb: usize) -> ! {
    x_println!("");
    x_println!(
        "\
        arch = {}\n\
        platform = {}\n\
        target = {}\n\
        build_mode = {}\n\
        log_level = {}\n\
        smp = {}\n\
        ",
        xconfig::ARCH,
        xconfig::PLATFORM,
        option_env!("XCORE_TARGET").unwrap_or(""),
        option_env!("XCORE_MODE").unwrap_or(""),
        option_env!("XCORE_LOG").unwrap_or(""),
        xconfig::SMP,
    );
    #[cfg(feature = "rtc")]
    x_println!(
        "Boot at {}\n",
        chrono::DateTime::from_timestamp_nanos(xhal::time::wall_time_nanos() as _),
    );

    xlog::init();
    xlog::set_max_level(option_env!("XCORE_LOG").unwrap_or("")); // no effect if set `log-level-*` features
    info!("Logging is enabled.");
    info!("Primary CPU {} started, dtb = {:#x}.", cpu_id, dtb);

    info!("Found physcial memory regions:");
    for r in xhal::mem::memory_regions() {
        info!(
            "  [{:x?}, {:x?}) {} ({:?})",
            r.paddr,
            r.paddr + r.size,
            r.name,
            r.flags
        );
    }

    #[cfg(feature = "alloc")]
    init_allocator();

    #[cfg(feature = "paging")]
    xmm::init_memory_management();

    info!("Initialize platform devices...");
    xhal::platform_init();

    #[cfg(feature = "multitask")]
    xtask::init_scheduler();

    #[cfg(any(feature = "fs", feature = "net", feature = "display"))]
    {
        #[allow(unused_variables)]
        let mut all_devices = xdriver::init_drivers();

        #[cfg(feature = "fs")]
        {
            #[allow(unused_imports)]
            use xdriver::prelude::BaseDriverOps as _;

            let dev = all_devices
                .block
                .take_one()
                .expect("No block device found!");
            info!("Block device: {}", dev.device_name());
            let fs = xfs::fs::new_default(dev).expect("Failed to initialize filesystem");
            let mount = xvfs::Mountpoint::new_root(&fs);
            xfs::FS_CONTEXT.init_new(xsync::Mutex::new(xfs::FsContext::new(
                mount.root_location(),
            )));
        }

        #[cfg(feature = "net")]
        xnet::init_network(all_devices.net);
        #[cfg(feature = "display")]
        xdisplay::init_display(all_devices.display);
    }

    #[cfg(feature = "smp")]
    self::mp::start_secondary_cpus(cpu_id);

    #[cfg(feature = "irq")]
    {
        info!("Initialize interrupt handlers...");
        init_interrupt();
    }

    #[cfg(all(feature = "tls", not(feature = "multitask")))]
    {
        info!("Initialize thread local storage...");
        init_tls();
    }

    ctor_bare::call_ctors();

    info!("Primary CPU {} init OK.", cpu_id);
    INITED_CPUS.fetch_add(1, Ordering::Relaxed);

    while !is_init_ok() {
        core::hint::spin_loop();
    }

    unsafe { main() };

    #[cfg(feature = "multitask")]
    xtask::exit(0);
    #[cfg(not(feature = "multitask"))]
    {
        debug!("main task exited: exit_code={}", 0);
        xhal::misc::terminate();
    }
}

#[cfg(feature = "alloc")]
fn init_allocator() {
    use xhal::mem::{MemRegionFlags, memory_regions, phys_to_virt};

    info!("Initialize global memory allocator...");
    info!("  use {} allocator.", xalloc::global_allocator().name());

    let mut max_region_size = 0;
    let mut max_region_paddr = 0.into();
    for r in memory_regions() {
        if r.flags.contains(MemRegionFlags::FREE) && r.size > max_region_size {
            max_region_size = r.size;
            max_region_paddr = r.paddr;
        }
    }
    for r in memory_regions() {
        if r.flags.contains(MemRegionFlags::FREE) && r.paddr == max_region_paddr {
            let start_vaddr = phys_to_virt(r.paddr).as_usize();
            xalloc::global_init(start_vaddr, r.size);
            break;
        }
    }
    for r in memory_regions() {
        if r.flags.contains(MemRegionFlags::FREE) && r.paddr != max_region_paddr {
            xalloc::global_add_memory(phys_to_virt(r.paddr).as_usize(), r.size)
                .expect("add heap memory region failed");
        }
    }
}

#[cfg(feature = "irq")]
fn init_interrupt() {
    use xhal::time::TIMER_IRQ_NUM;

    // Setup timer interrupt handler
    const PERIODIC_INTERVAL_NANOS: u64 = xhal::time::NANOS_PER_SEC / xconfig::TICKS_PER_SEC as u64;

    #[percpu::def_percpu]
    static NEXT_DEADLINE: u64 = 0;

    fn update_timer() {
        let now_ns = xhal::time::monotonic_time_nanos();
        // Safety: we have disabled preemption in IRQ handler.
        let mut deadline = unsafe { NEXT_DEADLINE.read_current_raw() };
        if now_ns >= deadline {
            deadline = now_ns + PERIODIC_INTERVAL_NANOS;
        }
        unsafe { NEXT_DEADLINE.write_current_raw(deadline + PERIODIC_INTERVAL_NANOS) };
        xhal::time::set_oneshot_timer(deadline);
    }

    xhal::irq::register_handler(TIMER_IRQ_NUM, || {
        update_timer();
        #[cfg(feature = "multitask")]
        xtask::on_timer_tick();
        // Refresh the vDSO data page (no-op on non-boot CPUs and if the
        // kernel did not register a vdso tick handler).
        crate_interface::call_interface!(VdsoTickIf::on_timer_tick);
    });

    // Enable IRQs before starting app
    xhal::arch::enable_irqs();
}

#[cfg(all(feature = "tls", not(feature = "multitask")))]
fn init_tls() {
    let main_tls = xhal::tls::TlsArea::alloc();
    unsafe { xhal::arch::write_thread_pointer(main_tls.tls_ptr() as usize) };
    core::mem::forget(main_tls);
}
