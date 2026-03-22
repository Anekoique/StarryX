use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    ax_println!("panic: {}", info);
    axhal::misc::terminate()
}
