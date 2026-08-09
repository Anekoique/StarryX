/// The maximum number of bytes that can be read at once.
const MAX_RW_SIZE: usize = 256;

/// Writes a byte to the console using the best available method.
pub fn putchar(c: u8) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c as usize);
}

/// Tries to write bytes to the console from input u8 slice.
/// Returns the number of bytes written.
fn try_write_bytes(bytes: &[u8]) -> usize {
    for &byte in bytes.iter().take(MAX_RW_SIZE) {
        #[allow(deprecated)]
        sbi_rt::legacy::console_putchar(byte as usize);
    }
    bytes.len().min(MAX_RW_SIZE)
}

/// Writes bytes to the console from input u8 slice.
pub fn write_bytes(bytes: &[u8]) {
    // If the address is from userspace, we need to copy the bytes to kernel space.
    #[cfg(feature = "uspace")]
    if bytes.as_ptr() as usize & (1 << 63) == 0 {
        // Check if the address is valid.
        let kernel_bytes = bytes.to_vec();
        let mut write_len = 0;
        while write_len < kernel_bytes.len() {
            let len = try_write_bytes(&kernel_bytes[write_len..]);
            if len == 0 {
                break;
            }
            write_len += len;
        }
        return;
    }
    let mut write_len = 0;
    while write_len < bytes.len() {
        let len = try_write_bytes(&bytes[write_len..]);
        if len == 0 {
            break;
        }
        write_len += len;
    }
}

/// Reads bytes from the console into the given mutable slice.
/// Returns the number of bytes read.
pub fn read_bytes(bytes: &mut [u8]) -> usize {
    let mut read_count = 0;
    for byte in bytes.iter_mut().take(MAX_RW_SIZE) {
        #[allow(deprecated)]
        let ch = sbi_rt::legacy::console_getchar();
        if ch == usize::MAX {
            // No more characters available
            break;
        }
        *byte = ch as u8;
        read_count += 1;
    }
    read_count
}
