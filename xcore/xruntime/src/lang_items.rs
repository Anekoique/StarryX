use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    x_println!("panic: {}", info);
    xhal::misc::terminate()
}
