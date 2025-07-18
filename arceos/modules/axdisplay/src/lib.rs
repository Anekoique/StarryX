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
use lazyinit::LazyInit;

static MAIN_DISPLAY: LazyInit<Mutex<AxDisplayDevice>> = LazyInit::new();

/// Initializes the graphics subsystem by underlayer devices.
pub fn init_display(mut display_devs: AxDeviceContainer<AxDisplayDevice>) {
    info!("Initialize graphics subsystem...");

    let dev = display_devs.take_one().expect("No graphics device found!");
    info!("  use graphics device 0: {:?}", dev.device_name());
    MAIN_DISPLAY.init_once(Mutex::new(dev));
}

/// Gets the framebuffer information.
pub fn framebuffer_info() -> DisplayInfo {
    MAIN_DISPLAY.lock().info()
}

/// Flushes the framebuffer, i.e. show on the screen.
pub fn framebuffer_flush() {
    MAIN_DISPLAY.lock().flush().unwrap();
}

