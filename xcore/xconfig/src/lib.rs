//! Platform-specific constants and parameters for [ArceOS].
//!
//! Currently supported platform configs can be found in the [configs] directory of
//! the [ArceOS] root.
//!
//! [ArceOS]: https://github.com/arceos-org/arceos
//! [configs]: https://github.com/arceos-org/arceos/tree/main/configs

#![no_std]

// The external macro expands code that refers to its published crate name.
// Keep that compatibility name private to this boundary.
extern crate xconfig_macros as axconfig_macros;

xconfig_macros::include_configs!(
    path_env = "XCORE_CONFIG_PATH",
    fallback = "../../configs/dummy.toml"
);
