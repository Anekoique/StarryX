//! Defines types and probe methods of all supported devices.

#![allow(unused_imports, dead_code)]

use crate::XDeviceEnum;
use xdriver_base::DeviceType;

#[cfg(feature = "virtio")]
use crate::virtio::{self, VirtIoDevMeta};

#[cfg(feature = "bus-pci")]
use xdriver_pci::{DeviceFunction, DeviceFunctionInfo, PciRoot};

pub use super::dummy::*;

pub trait DriverProbe {
    fn probe_global() -> Option<XDeviceEnum> {
        None
    }

    #[cfg(bus = "mmio")]
    fn probe_mmio(_mmio_base: usize, _mmio_size: usize) -> Option<XDeviceEnum> {
        None
    }

    #[cfg(bus = "pci")]
    fn probe_pci(
        _root: &mut PciRoot,
        _bdf: DeviceFunction,
        _dev_info: &DeviceFunctionInfo,
    ) -> Option<XDeviceEnum> {
        None
    }
}

#[cfg(net_dev = "virtio-net")]
register_net_driver!(
    <virtio::VirtIoNet as VirtIoDevMeta>::Driver,
    <virtio::VirtIoNet as VirtIoDevMeta>::Device
);

#[cfg(block_dev = "virtio-blk")]
register_block_driver!(
    <virtio::VirtIoBlk as VirtIoDevMeta>::Driver,
    <virtio::VirtIoBlk as VirtIoDevMeta>::Device
);

#[cfg(display_dev = "virtio-gpu")]
register_display_driver!(
    <virtio::VirtIoGpu as VirtIoDevMeta>::Driver,
    <virtio::VirtIoGpu as VirtIoDevMeta>::Device
);

cfg_if::cfg_if! {
    if #[cfg(block_dev = "ramdisk")] {
        pub struct RamDiskDriver;
        register_block_driver!(RamDiskDriver, xdriver_block::ramdisk::RamDisk);

        #[macro_export]
        macro_rules! init_ramdisk {
            ($path:literal) => {
                core::arch::global_asm!(
                    concat!(
                        ".section .data
                        .global initrd_start
                        .global initrd_end
                        .p2align 12
                        initrd_start:
                        .incbin \"",
                        $path,
                        "\"
                        initrd_end:"
                    )
                );
            }
        }

        impl DriverProbe for RamDiskDriver {
            fn probe_global() -> Option<XDeviceEnum> {
                unsafe extern "C" {
                    fn initrd_start();
                    fn initrd_end();
                }

                let initrd = unsafe {
                    xdriver_block::ramdisk::RamDisk::new(
                        initrd_start as *const () as usize,
                        initrd_end as *const () as usize - initrd_start as *const () as usize,
                    )
                };
                Some(XDeviceEnum::from_block(initrd))
            }
        }

    }
}

cfg_if::cfg_if! {
    if #[cfg(block_dev = "bcm2835-sdhci")]{
        pub struct BcmSdhciDriver;
        register_block_driver!(MmckDriver, xdriver_block::bcm2835sdhci::SDHCIDriver);

        impl DriverProbe for BcmSdhciDriver {
            fn probe_global() -> Option<XDeviceEnum> {
                debug!("mmc probe");
                xdriver_block::bcm2835sdhci::SDHCIDriver::try_new().ok().map(XDeviceEnum::from_block)
            }
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(block_dev = "visionfive2-sd")] {
        pub struct SdDriver;
        register_block_driver!(SdDriver, xdriver_block::visionfive2sd::VF2SD);

        impl DriverProbe for SdDriver {
            fn probe_global() -> Option<XDeviceEnum> {
                Some(XDeviceEnum::from_block(
                    xdriver_block::visionfive2sd::VF2SD::new(),
                ))
            }
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(net_dev = "ixgbe")] {
        use crate::ixgbe::IxgbeHalImpl;
        use xhal::mem::phys_to_virt;
        pub struct IxgbeDriver;
        register_net_driver!(IxgbeDriver, xdriver_net::ixgbe::IxgbeNic<IxgbeHalImpl, 1024, 1>);
        impl DriverProbe for IxgbeDriver {
            #[cfg(bus = "pci")]
            fn probe_pci(
                    root: &mut xdriver_pci::PciRoot,
                    bdf: xdriver_pci::DeviceFunction,
                    dev_info: &xdriver_pci::DeviceFunctionInfo,
                ) -> Option<crate::XDeviceEnum> {
                    use xdriver_net::ixgbe::{INTEL_82599, INTEL_VEND, IxgbeNic};
                    if dev_info.vendor_id == INTEL_VEND && dev_info.device_id == INTEL_82599 {
                        // Intel 10Gb Network
                        info!("ixgbe PCI device found at {:?}", bdf);

                        // Initialize the device
                        // These can be changed according to the requirments specified in the ixgbe init function.
                        const QN: u16 = 1;
                        const QS: usize = 1024;
                        let bar_info = root.bar_info(bdf, 0).unwrap();
                        match bar_info {
                            xdriver_pci::BarInfo::Memory {
                                address,
                                size,
                                ..
                            } => {
                                let ixgbe_nic = IxgbeNic::<IxgbeHalImpl, QS, QN>::init(
                                    phys_to_virt((address as usize).into()).into(),
                                    size as usize
                                )
                                .expect("failed to initialize ixgbe device");
                                return Some(XDeviceEnum::from_net(ixgbe_nic));
                            }
                            xdriver_pci::BarInfo::IO { .. } => {
                                error!("ixgbe: BAR0 is of I/O type");
                                return None;
                            }
                        }
                    }
                    None
            }
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(net_dev = "fxmac")]{
        use xalloc::global_allocator;
        use xhal::mem::PAGE_SIZE_4K;

        #[crate_interface::impl_interface]
        impl xdriver_net::fxmac::KernelFunc for FXmacDriver {
            fn virt_to_phys(addr: usize) -> usize {
                xhal::mem::virt_to_phys(addr.into()).into()
            }

            fn phys_to_virt(addr: usize) -> usize {
                xhal::mem::phys_to_virt(addr.into()).into()
            }

            fn dma_alloc_coherent(pages: usize) -> (usize, usize) {
                let Ok(vaddr) = global_allocator().alloc_pages(pages, PAGE_SIZE_4K) else {
                    error!("failed to alloc pages");
                    return (0, 0);
                };
                let paddr = xhal::mem::virt_to_phys((vaddr).into());
                debug!("alloc pages @ vaddr={:#x}, paddr={:#x}", vaddr, paddr);
                (vaddr, paddr.as_usize())
            }

            fn dma_free_coherent(vaddr: usize, pages: usize) {
                global_allocator().dealloc_pages(vaddr, pages);
            }

            fn dma_request_irq(_irq: usize, _handler: fn()) {
                warn!("unimplemented dma_request_irq for fxmax");
            }
        }

        register_net_driver!(FXmacDriver, xdriver_net::fxmac::FXmacNic);

        pub struct FXmacDriver;
        impl DriverProbe for FXmacDriver {
            fn probe_global() -> Option<XDeviceEnum> {
                info!("fxmac for phytiumpi probe global");
                xdriver_net::fxmac::FXmacNic::init(0).ok().map(XDeviceEnum::from_net)
            }
        }
    }
}

#[cfg(feature = "ramdisk")]
init_ramdisk!("./sdcard-rv.img");
