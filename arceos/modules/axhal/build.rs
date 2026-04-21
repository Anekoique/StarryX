use std::io::Result;
use std::path::Path;

const BUILTIN_PLATFORMS: &[&str] = &[
    "loongarch64-qemu-virt",
    "riscv64-qemu-virt",
    "riscv64-visionfive2",
];

const BUILTIN_PLATFORM_FAMILIES: &[&str] = &[
    "loongarch64-qemu-virt",
    "riscv64-qemu-virt",
    "riscv64-visionfive2",
];

fn make_cfg_values(str_list: &[&str]) -> String {
    str_list
        .iter()
        .map(|s| format!("{:?}", s))
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let platform = axconfig::PLATFORM;
    if platform != "dummy" {
        gen_linker_script(&arch, platform).unwrap();
    }

    println!("cargo:rustc-cfg=platform=\"{}\"", platform);
    println!(
        "cargo:rustc-cfg=platform_family=\"{}\"",
        axconfig::plat::FAMILY
    );
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values({}))",
        make_cfg_values(BUILTIN_PLATFORMS)
    );
    println!(
        "cargo::rustc-check-cfg=cfg(platform_family, values({}))",
        make_cfg_values(BUILTIN_PLATFORM_FAMILIES)
    );
}

fn gen_linker_script(arch: &str, platform: &str) -> Result<()> {
    let fname = format!("linker_{}.lds", platform);
    // BFD names: riscv64 -> "riscv", loongarch64 -> "loongarch64".
    let output_arch = if arch.contains("riscv") { "riscv" } else { arch };
    let ld_content = std::fs::read_to_string("linker.lds.S")?;
    let ld_content = ld_content.replace("%ARCH%", output_arch);
    let ld_content = ld_content.replace(
        "%KERNEL_BASE%",
        &format!("{:#x}", axconfig::plat::KERNEL_BASE_VADDR),
    );
    let ld_content = ld_content.replace(
        "%KERNEL_BASE_PADDR%",
        &format!("{:#x}", axconfig::plat::KERNEL_BASE_PADDR),
    );
    let ld_content = ld_content.replace("%SMP%", &format!("{}", axconfig::SMP));

    // target/<target_triple>/<mode>/build/axhal-xxxx/out
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // target/<target_triple>/<mode>/linker_xxxx.lds
    let out_path = Path::new(&out_dir).join("../../..").join(fname);
    std::fs::write(out_path, ld_content)?;
    Ok(())
}
