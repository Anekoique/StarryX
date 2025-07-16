//! [ArceOS](https://github.com/arceos-org/arceos) graphics module.
//!
//! Currently supports direct writing to the framebuffer.
//! With optional embedded-graphics integration for advanced graphics capabilities.

#![no_std]

#[macro_use]
extern crate log;

#[doc(no_inline)]
pub use axdriver_display::DisplayInfo;

#[cfg(feature = "embedded-graphics")]
pub mod graphics;

#[cfg(feature = "embedded-graphics")]
pub use graphics::{AxFrameBuffer, GraphicsRenderer};

#[cfg(feature = "embedded-graphics")]
pub mod prelude {
    //! 预导入模块
    //! 
    //! 包含常用的embedded-graphics类型和ArceOS扩展。
    
    pub use crate::graphics::{AxFrameBuffer, GraphicsRenderer};
    pub use embedded_graphics::{
        geometry::{Point, Size},
        pixelcolor::{Rgb565, RgbColor},
        prelude::*,
        primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    };
}

use axdriver::{AxDeviceContainer, prelude::*};
use axsync::Mutex;

static MAIN_DISPLAY: Mutex<Option<AxDisplayDevice>> = Mutex::new(None);

/// Initializes the graphics subsystem by underlayer devices.
pub fn init_display(mut display_devs: AxDeviceContainer<AxDisplayDevice>) {
    info!("Initialize graphics subsystem...");

    let dev = display_devs.take_one().expect("No graphics device found!");
    info!("  use graphics device 0: {:?}", dev.device_name());
    
    // 直接设置设备，不需要 LazyInit
    *MAIN_DISPLAY.lock() = Some(dev);
}

/// Checks if the display has been initialized.
pub fn is_display_initialized() -> bool {
    MAIN_DISPLAY.lock().is_some()
}

/// Gets the framebuffer information.
/// Returns a default DisplayInfo with width=0 if the display is not yet initialized.
pub fn framebuffer_info() -> DisplayInfo {
    if let Some(ref dev) = *MAIN_DISPLAY.lock() {
        dev.info()
    } else {
        // Return a default DisplayInfo indicating the display is not ready
        DisplayInfo {
            width: 0,
            height: 0,
            fb_base_vaddr: 0,
            fb_size: 0,
        }
    }
}

/// Flushes the framebuffer, i.e. show on the screen.
/// Does nothing if the display is not initialized.
pub fn framebuffer_flush() {
    if let Some(ref mut dev) = MAIN_DISPLAY.lock().as_mut() {
        dev.flush().unwrap();
    }
}

