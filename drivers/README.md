# xdriver_crates

Crates for building device driver subsystems in the `no_std` environment:

- [xdriver_base](https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_base): Common interfaces for all kinds of device drivers.
- [xdriver_block](https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_block): Common traits and types for block storage drivers.
- [xdriver_net](https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_net): Common traits and types for network device (NIC) drivers.
- [xdriver_display](https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_display): Common traits and types for graphics device drivers.
- [xdriver_pci](https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_pci): Structures and functions for PCI bus operations.
- [xdriver_virtio](https://github.com/arceos-org/axdriver_crates/tree/main/axdriver_virtio): Wrappers of some devices in the [virtio-drivers](https://docs.rs/virtio-drivers) crate, that implement traits in the local `xdriver` series.
