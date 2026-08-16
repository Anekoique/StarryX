use std::{
    alloc::{self, Layout},
    cell::RefCell,
    collections::HashSet,
    marker::PhantomData,
};

use memory_addr::{PhysAddr, VirtAddr};
use page_table_entry::{GenericPTE, MappingFlags};
use page_table_multiarch::{PageSize, PageTable64, PagingHandler, PagingMetaData, PagingResult};
use rand::{Rng, SeedableRng, rngs::SmallRng};

const PAGE_LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked(4096, 4096) };

thread_local! {
    static ALLOCATED: RefCell<HashSet<usize>> = RefCell::default();
}

struct TrackPagingHandler<M: PagingMetaData>(PhantomData<M>);

impl<M: PagingMetaData> PagingHandler for TrackPagingHandler<M> {
    fn alloc_frame() -> Option<PhysAddr> {
        let ptr = unsafe { alloc::alloc(PAGE_LAYOUT) } as usize;
        assert!(
            ptr <= M::PA_MAX_ADDR,
            "allocated frame address exceeds PA_MAX_ADDR"
        );
        ALLOCATED.with_borrow_mut(|it| it.insert(ptr));
        Some(PhysAddr::from_usize(ptr))
    }

    fn dealloc_frame(paddr: PhysAddr) {
        let ptr = paddr.as_usize();
        ALLOCATED.with_borrow_mut(|it| {
            assert!(it.remove(&ptr), "dealloc a frame that was not allocated");
        });
        unsafe {
            alloc::dealloc(ptr as _, PAGE_LAYOUT);
        }
    }

    fn phys_to_virt(paddr: PhysAddr) -> VirtAddr {
        assert!(paddr.as_usize() > 0);
        VirtAddr::from_usize(paddr.as_usize())
    }
}

fn run_test_for<M: PagingMetaData<VirtAddr = VirtAddr>, PTE: GenericPTE>() -> PagingResult<()> {
    ALLOCATED.with_borrow_mut(|it| {
        it.clear();
    });

    let vaddr_mask = ((1u64 << M::VA_MAX_BITS) - 1) & !0xfff;

    let mut table = PageTable64::<M, PTE, TrackPagingHandler<M>>::try_new().unwrap();
    let mut pages = HashSet::new();
    let mut rng = SmallRng::seed_from_u64(1234);
    for _ in 0..2048 {
        if rng.random_ratio(3, 4) || pages.is_empty() {
            // insert a mapping
            let addr = loop {
                let addr = rng.random::<u64>() & vaddr_mask;
                if pages.insert(addr) {
                    break addr;
                }
            };
            table
                .map(
                    VirtAddr::from_usize(addr as usize),
                    PhysAddr::from_usize((rng.random::<u64>() & vaddr_mask) as usize),
                    PageSize::Size4K,
                    MappingFlags::READ | MappingFlags::WRITE,
                )?
                .ignore();
        } else {
            // remove a mapping
            let addr = *pages.iter().next().unwrap();
            table.unmap(VirtAddr::from_usize(addr as usize))?.2.ignore();
            pages.remove(&addr);
        }
    }

    drop(table);
    assert_eq!(
        ALLOCATED.with_borrow(|it| it.len()),
        0,
        "Some frames were not deallocated"
    );

    Ok(())
}

fn run_leaf_range_test<M: PagingMetaData<VirtAddr = VirtAddr>, PTE: GenericPTE>() {
    ALLOCATED.with_borrow_mut(|it| it.clear());
    let mut table = PageTable64::<M, PTE, TrackPagingHandler<M>>::try_new().unwrap();
    for address in [0x1000, 0x4000, 0x20_0000] {
        let flags = if address == 0x1000 {
            MappingFlags::READ | MappingFlags::ALLOC_FRAME
        } else {
            MappingFlags::READ
        };
        table
            .map(
                VirtAddr::from_usize(address),
                PhysAddr::from_usize(0x1000_0000 + address),
                PageSize::Size4K,
                flags,
            )
            .unwrap()
            .ignore();
    }

    let mut leaves = Vec::new();
    table
        .walk_leaf_range(VirtAddr::from_usize(0x1000), 0x5000, |address, _, _, _| {
            leaves.push(address.as_usize());
        })
        .unwrap();
    assert_eq!(leaves, [0x1000, 0x4000]);
    assert!(
        table
            .query(VirtAddr::from_usize(0x1000))
            .unwrap()
            .1
            .contains(MappingFlags::ALLOC_FRAME)
    );
    assert!(
        !table
            .query(VirtAddr::from_usize(0x4000))
            .unwrap()
            .1
            .contains(MappingFlags::ALLOC_FRAME)
    );

    let high_address = usize::MAX - 0x1fff;
    table
        .map(
            VirtAddr::from_usize(high_address),
            PhysAddr::from_usize(0x2000_0000),
            PageSize::Size4K,
            MappingFlags::READ,
        )
        .unwrap()
        .ignore();
    let mut high_leaves = Vec::new();
    table
        .walk_leaf_range(
            VirtAddr::from_usize(high_address),
            PageSize::Size4K as usize,
            |address, _, _, _| high_leaves.push(address.as_usize()),
        )
        .unwrap();
    assert_eq!(high_leaves, [high_address]);

    drop(table);
    assert_eq!(ALLOCATED.with_borrow(|it| it.len()), 0);
}

fn run_prot_none_test<M: PagingMetaData<VirtAddr = VirtAddr>, PTE: GenericPTE>() {
    ALLOCATED.with_borrow_mut(|it| it.clear());
    let mut table = PageTable64::<M, PTE, TrackPagingHandler<M>>::try_new().unwrap();
    let address = VirtAddr::from_usize(0x4000);
    let physical = PhysAddr::from_usize(0x1000_0000);
    let alloc_flags = MappingFlags::READ | MappingFlags::USER | MappingFlags::ALLOC_FRAME;

    table
        .map(address, physical, PageSize::Size4K, alloc_flags)
        .unwrap()
        .ignore();
    table
        .protect(address, MappingFlags::PROT_NONE | MappingFlags::ALLOC_FRAME)
        .unwrap()
        .1
        .ignore();

    let (mapped, flags, size) = table.query(address).unwrap();
    assert_eq!(mapped, physical);
    assert_eq!(size, PageSize::Size4K);
    assert!(flags.contains(MappingFlags::PROT_NONE));
    assert!(flags.contains(MappingFlags::ALLOC_FRAME));
    assert!(!flags.intersects(MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE));

    let mut leaves = Vec::new();
    table
        .walk_leaf_range(
            address,
            PageSize::Size4K.into(),
            |virtual_address, _, _, _| leaves.push(virtual_address),
        )
        .unwrap();
    assert_eq!(leaves, [address]);

    table.protect(address, alloc_flags).unwrap().1.ignore();
    assert_eq!(table.query(address).unwrap().1, alloc_flags);
    let (unmapped, size, tlb) = table.unmap(address).unwrap();
    tlb.ignore();
    assert_eq!(unmapped, physical);
    assert_eq!(size, PageSize::Size4K);

    let huge_address = VirtAddr::from_usize(0x20_0000);
    let huge_physical = PhysAddr::from_usize(0x2000_0000);
    table
        .map(huge_address, huge_physical, PageSize::Size2M, alloc_flags)
        .unwrap()
        .ignore();
    table
        .protect(
            huge_address,
            MappingFlags::PROT_NONE | MappingFlags::ALLOC_FRAME,
        )
        .unwrap()
        .1
        .ignore();
    assert_eq!(
        table.query(huge_address).unwrap(),
        (
            huge_physical,
            MappingFlags::PROT_NONE | MappingFlags::ALLOC_FRAME,
            PageSize::Size2M,
        )
    );
    table.unmap(huge_address).unwrap().2.ignore();

    drop(table);
    assert_eq!(ALLOCATED.with_borrow(|it| it.len()), 0);
}

#[cfg(target_arch = "x86_64")]
fn run_x86_prot_none_encoding_test() {
    use page_table_entry::x86_64::X64PTE;

    const PHYS_ADDR_MASK: usize = 0x000f_ffff_ffff_f000;
    let flags = MappingFlags::PROT_NONE | MappingFlags::ALLOC_FRAME;
    let physical = PhysAddr::from_usize(0x1234_5000);
    let mut pte = X64PTE::new_page(physical, flags, false);

    assert_eq!(pte.paddr(), physical);
    assert_eq!(
        pte.bits() & PHYS_ADDR_MASK,
        !physical.as_usize() & PHYS_ADDR_MASK
    );

    let replacement = PhysAddr::from_usize(0x5678_9000);
    pte.set_paddr(replacement);
    assert_eq!(pte.paddr(), replacement);
    assert_eq!(
        pte.bits() & PHYS_ADDR_MASK,
        !replacement.as_usize() & PHYS_ADDR_MASK
    );

    pte.set_flags(MappingFlags::READ | MappingFlags::ALLOC_FRAME, false);
    assert_eq!(pte.paddr(), replacement);
    assert_eq!(pte.bits() & PHYS_ADDR_MASK, replacement.as_usize());
}

#[test]
#[cfg(target_arch = "x86_64")]
fn test_dealloc_x86() -> PagingResult<()> {
    run_test_for::<
        page_table_multiarch::x86_64::X64PagingMetaData,
        page_table_entry::x86_64::X64PTE,
    >()?;
    run_prot_none_test::<
        page_table_multiarch::x86_64::X64PagingMetaData,
        page_table_entry::x86_64::X64PTE,
    >();
    run_x86_prot_none_encoding_test();
    Ok(())
}

#[test]
#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
fn test_dealloc_riscv() -> PagingResult<()> {
    run_test_for::<
        page_table_multiarch::riscv::Sv39MetaData<VirtAddr>,
        page_table_entry::riscv::Rv64PTE,
    >()?;
    run_test_for::<
        page_table_multiarch::riscv::Sv48MetaData<VirtAddr>,
        page_table_entry::riscv::Rv64PTE,
    >()?;
    run_prot_none_test::<
        page_table_multiarch::riscv::Sv39MetaData<VirtAddr>,
        page_table_entry::riscv::Rv64PTE,
    >();
    Ok(())
}

#[test]
#[cfg(target_arch = "aarch64")]
fn test_dealloc_aarch64() -> PagingResult<()> {
    run_test_for::<
        page_table_multiarch::aarch64::A64PagingMetaData,
        page_table_entry::aarch64::A64PTE,
    >()?;
    run_leaf_range_test::<
        page_table_multiarch::aarch64::A64PagingMetaData,
        page_table_entry::aarch64::A64PTE,
    >();
    run_prot_none_test::<
        page_table_multiarch::aarch64::A64PagingMetaData,
        page_table_entry::aarch64::A64PTE,
    >();
    Ok(())
}

#[test]
#[cfg(target_arch = "loongarch64")]
fn test_dealloc_loongarch64() -> PagingResult<()> {
    run_test_for::<
        page_table_multiarch::loongarch64::LA64MetaData,
        page_table_entry::loongarch64::LA64PTE,
    >()?;
    run_leaf_range_test::<
        page_table_multiarch::loongarch64::LA64MetaData,
        page_table_entry::loongarch64::LA64PTE,
    >();
    run_prot_none_test::<
        page_table_multiarch::loongarch64::LA64MetaData,
        page_table_entry::loongarch64::LA64PTE,
    >();
    Ok(())
}
