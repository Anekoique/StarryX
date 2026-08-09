use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=XCORE_CONFIG_PATH");
    if let Ok(config_path) = std::env::var("XCORE_CONFIG_PATH") {
        println!("cargo:rerun-if-changed={}", config_path);
    } else {
        let root_path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let dummy_config = Path::new(&root_path).join("../../configs/dummy.toml");
        println!(
            "cargo:rustc-env=XCORE_CONFIG_PATH={}",
            dummy_config.display()
        );
    }
}
