// build.rs — drive per-arch linker script + version script + soname.
//
// The linker script lives under `linker/` next to this build.rs; the
// version script is shared across arches.

use std::env;
use std::path::PathBuf;

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH unset");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linker_dir = manifest_dir.join("linker");

    let arch_lds = match arch.as_str() {
        "riscv64" => linker_dir.join("vdso-riscv64.lds"),
        "loongarch64" => linker_dir.join("vdso-loongarch64.lds"),
        other => panic!("xvdso: unsupported target arch `{other}`"),
    };
    let version_lds = linker_dir.join("vdso-version.lds");

    println!("cargo:rerun-if-changed={}", arch_lds.display());
    println!("cargo:rerun-if-changed={}", version_lds.display());

    println!("cargo:rustc-link-arg=-T{}", arch_lds.display());
    println!(
        "cargo:rustc-link-arg=--version-script={}",
        version_lds.display()
    );
    println!("cargo:rustc-link-arg=-soname=linux-vdso.so.1");
    println!("cargo:rustc-link-arg=--build-id=none");
    println!("cargo:rustc-link-arg=--hash-style=both");
    println!("cargo:rustc-link-arg=-z");
    println!("cargo:rustc-link-arg=max-page-size=0x1000");
    println!("cargo:rustc-link-arg=--no-eh-frame-hdr");
    println!("cargo:rustc-link-arg=-nostdlib");
}
