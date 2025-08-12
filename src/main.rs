#![no_std]
#![no_main]
#![doc = include_str!("../README.md")]

#[macro_use]
extern crate axlog;
extern crate alloc;
extern crate axruntime;

use alloc::{format, string::ToString, vec::Vec};

mod entry;
mod mm;
mod syscall;

const LOGO: &str = r#"
 .d8888b.  888                                      Y88b   d88P
d88P  Y88b 888                                       Y88b d88P
Y88b.      888                                        Y88o88P
 "Y888b.   888888  8888b.  888d88 8888d888 888  888    Y888P
    "Y88b. 888        "88b 888P"   888P"   888  888    d888b
      "888 888    .d888888 888     888     888  888   d88888b
Y88b  d88P Y88b.  888  888 888     888     Y88b 888  d88P Y88b
 "Y8888P"   "Y888 "Y888888 888     888      "Y88888 d88P   Y88b
                                                888
                                           Y8b d88P
                                            "Y88P"
"#;

const RAINBOW_COLORS: &[&str] = &[
    "\x1b[38;5;219m",
    "\x1b[38;5;217m",
    "\x1b[38;5;216m",
    "\x1b[38;5;229m",
    "\x1b[38;5;193m",
    "\x1b[38;5;158m",
    "\x1b[38;5;159m",
    "\x1b[38;5;153m",
    "\x1b[38;5;147m",
    "\x1b[38;5;183m",
    "\x1b[38;5;182m",
    "\x1b[38;5;218m",
];

const RESET_COLOR: &str = "\x1b[0m";

fn print_logo() {
    let lines: Vec<&str> = LOGO.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let color = RAINBOW_COLORS[i % RAINBOW_COLORS.len()];
        ax_println!("{}{}{}", color, line, RESET_COLOR);
    }
}

#[unsafe(no_mangle)]
fn main() {
    print_logo();
    xprocess::Process::new_init(axtask::current().id().as_u64() as _).build();
    xcore::fs::vfs::init_root().expect("Failed to mount vfs");
    xcore::fs::fd::init_stdio().expect("Failed to init stdio");

    let envs = [format!("ARCH={}", option_env!("ARCH").unwrap_or("unknown"))];

    let init = include_str!("init.sh");

    info!("Running init script");
    let args = ["/musl/busybox", "sh", "-c", init]
        .map(|s| s.to_string())
        .to_vec();
    let exit_code = entry::run_user_app(&args, &envs);
    info!("Init script exited with code: {:?}", exit_code);
}
