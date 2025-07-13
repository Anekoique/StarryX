#![no_std]
#![no_main]
#![doc = include_str!("../README.md")]

#[macro_use]
extern crate axlog;
extern crate alloc;
extern crate axruntime;

use alloc::{format, string::ToString};

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

#[unsafe(no_mangle)]
fn main() {
    ax_println!("{}", LOGO);
    // Create a init process
    axprocess::Process::new_init(axtask::current().id().as_u64() as _).build();
    xcore::fs::init_root().expect("Failed to mount vfs");

    if option_env!("AX_TESTCASE") == Some("oscomp") {
        let envs = [format!("ARCH={}", option_env!("ARCH").unwrap_or("unknown"))];

        let init = include_str!("init.sh");

        info!("Running init script");
        let args = ["/musl/busybox", "sh", "-c", init]
            .map(|s| s.to_string())
            .to_vec();
        let exit_code = entry::run_user_app(&args, &envs);
        info!("Init script exited with code: {:?}", exit_code);
    } else {
        let testcases = option_env!("AX_TESTCASES_LIST")
            .unwrap_or_else(|| "Please specify the testcases list by making user_apps")
            .split(',');

        for testcase in testcases {
            let Some(args) = shlex::split(testcase) else {
                error!("Failed to parse testcase: {:?}", testcase);
                continue;
            };
            if args.is_empty() {
                continue;
            }
            info!("Running user task: {:?}", args);
            let exit_code = entry::run_user_app(&args, &[]);
            info!("User task {:?} exited with code: {:?}", args, exit_code);
        }
    }
}
