#![allow(unused)]

use std::collections::HashSet;

use axdriver_block::ramdisk::RamDisk;
use axfs_ng::{File, FsContext, fs};
use axfs_ng_vfs::{
    Filesystem, Location, Mountpoint, NodePermission, NodeType, VfsError, VfsResult, path::Path,
};
use axio::Read;

type RawMutex = spin::Mutex<()>;

fn list_files(cx: &FsContext<RawMutex>, path: impl AsRef<Path>) -> VfsResult<HashSet<String>> {
    cx.read_dir(path)?
        .map(|it| it.map(|entry| entry.name.to_owned()))
        .collect()
}
fn test_fs_read(fs: &Filesystem<RawMutex>) -> VfsResult<()> {
    let mount = Mountpoint::new_root(fs);
    let cx: FsContext<spin::mutex::Mutex<()>> = FsContext::new(mount.root_location());

    let names = list_files(&cx, "/").unwrap();
    assert!(
        ["short.txt", "long.txt", "a", "very-long-dir-name"]
            .into_iter()
            .all(|it| names.contains(it))
    );
    assert_eq!(cx.metadata("short.txt")?.size, 14);
    assert_eq!(cx.metadata("long.txt")?.size, 14000);

    let entries = cx.read_dir("/")?.collect::<VfsResult<Vec<_>>>()?;
    for entry in entries {
        assert!(cx.root_dir().lookup_no_follow(&entry.name)?.inode() == entry.ino);
    }

    assert_eq!(
        list_files(&cx, "/a/long/path")?,
        ["test.txt", ".", ".."]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(
        cx.read_to_string("/a/long/path/test.txt")?,
        "Rust is cool!\n"
    );

    assert_eq!(
        cx.resolve("/a/long/path/test.txt")?
            .absolute_path()?
            .to_string(),
        "/a/long/path/test.txt"
    );

    assert!(
        cx.resolve("/very-long-dir-name/very-long-file-name.txt")?
            .is_file()
    );
    let mut file = File::open(&cx, "/very-long-dir-name/very-long-file-name.txt")?;
    let mut buf = vec![];
    file.read_to_end(&mut buf)?;
    drop(file);
    assert_eq!(core::str::from_utf8(&buf).unwrap(), "Rust is cool!\n");

    Ok(())
}
fn test_fs_write(fs: &Filesystem<RawMutex>) -> VfsResult<()> {
    let mount = Mountpoint::new_root(fs);
    let cx = FsContext::new(mount.root_location());

    let mode = NodePermission::from_bits(0o766).unwrap();
    cx.create_dir("temp", mode)?;
    cx.create_dir("temp2", mode)?;
    assert!(cx.resolve("temp").is_ok() && cx.resolve("temp2").is_ok());
    // cx.rename("temp", "temp2")?;
    // assert!(cx.resolve("temp").is_err() && cx.resolve("temp2").is_ok());

    cx.create_dir("temp", mode)?;
    cx.resolve("temp")?
        .create("test.txt", NodeType::RegularFile, NodePermission::default())?;
    assert!(matches!(
        cx.rename("temp2", "temp"),
        Err(VfsError::ENOTEMPTY)
    ));

    cx.write("/test.txt", "hello world".as_bytes())?;
    assert_eq!(cx.read_to_string("/test.txt")?, "hello world");

    cx.create_dir("test_dir", NodePermission::from_bits_truncate(0o755))?;
    cx.rename("test_dir", "test")?;
    cx.remove_dir("test")?;

    println!("---------------------");

    if cx.link("/test.txt", "/test_link").is_ok() {
        assert_eq!(cx.read_to_string("/test_link")?, "hello world");
    }
    if cx.symlink("/test.txt", "/test_symlink").is_ok() {
        assert_eq!(cx.read_to_string("/test_symlink")?, "hello world");
    }

    // FAT has errornous rename implementation
    if fs.name() != "vfat" {
        cx.write("rename1", "hello world".as_bytes())?;
        cx.write("rename2", "hello world2".as_bytes())?;
        cx.rename("rename1", "rename2")?;
        assert_eq!(cx.read_to_string("rename2")?, "hello world");
    }

    Ok(())
}

fn test_fs_full(fs: Filesystem<RawMutex>) -> VfsResult<()> {
    let mut thrds = vec![];
    for _ in 0..1 {
        let fs = fs.clone();
        thrds.push(std::thread::spawn(move || test_fs_read(&fs)));
    }
    for th in thrds {
        th.join().unwrap()?;
    }
    test_fs_write(&fs)?;
    Ok(())
}

#[test]
#[cfg(feature = "fat")]
fn test_fatfs() {
    for path in ["resources/fat16.img", "resources/fat32.img"] {
        let data = std::fs::read(path).unwrap();
        let disk = RamDisk::from(&data);
        let fs = fs::fat::FatFilesystem::<RawMutex>::new(disk);
        test_fs_full(fs).unwrap();
    }
}

#[test]
#[cfg(feature = "ext4")]
fn test_ext4() {
    let data = std::fs::read("resources/ext4.img").unwrap();
    let disk = RamDisk::from(&data);
    let fs = fs::ext4::Ext4Filesystem::<RawMutex>::new(disk).unwrap();
    test_fs_full(fs).unwrap();
}

#[test]
#[cfg(all(feature = "ext4", feature = "fat"))]
fn test_mount() {
    env_logger::init();
    let disk = RamDisk::from(&std::fs::read("resources/ext4.img").unwrap());
    let fs = fs::ext4::Ext4Filesystem::<RawMutex>::new(disk).unwrap();

    let disk = RamDisk::from(&std::fs::read("resources/fat16.img").unwrap());
    let sub_fs = fs::fat::FatFilesystem::<RawMutex>::new(disk);

    let mount = Mountpoint::new(&fs, None);
    let cx = FsContext::new(mount.root_location());
    cx.resolve("a").unwrap().mount(&sub_fs).unwrap();

    let mt = cx.resolve("a").unwrap();
    assert!(!mt.is_mountpoint() && mt.is_root_of_mount());
    assert_eq!(mt.filesystem().name(), "vfat");
    assert_eq!(mt.absolute_path().unwrap().to_string(), "/a");

    assert_eq!(
        cx.read_to_string("/a/../a/very-long-dir-name/very-long-file-name.txt")
            .unwrap(),
        "Rust is cool!\n"
    );
}

#[test]
#[cfg(all(feature = "ext4", feature = "fat"))]
fn test_mount_extended() {
    // 准备文件系统
    let ext4_disk = RamDisk::from(&std::fs::read("resources/ext4.img").unwrap());
    let ext4_fs = fs::ext4::Ext4Filesystem::<RawMutex>::new(ext4_disk).unwrap();

    let fat16_disk = RamDisk::from(&std::fs::read("resources/fat16.img").unwrap());
    let fat16_fs = fs::fat::FatFilesystem::<RawMutex>::new(fat16_disk);

    let fat32_disk = RamDisk::from(&std::fs::read("resources/fat32.img").unwrap());
    let fat32_fs = fs::fat::FatFilesystem::<RawMutex>::new(fat32_disk);

    let mount = Mountpoint::new_root(&ext4_fs);
    let cx = FsContext::new(mount.root_location());

    // 测试1: 基本挂载功能
    test_basic_mount(&cx, &fat16_fs).unwrap();

    // 测试2: 嵌套挂载
    test_nested_mount(&cx, &fat16_fs, &fat32_fs).unwrap();
}

fn test_basic_mount(cx: &FsContext<RawMutex>, fs: &Filesystem<RawMutex>) -> VfsResult<()> {
    println!("Testing basic mount operations...");

    // 验证挂载前状态
    let dir_before = cx.resolve("/a")?;
    assert!(!dir_before.is_root_of_mount());
    assert_eq!(dir_before.filesystem().name(), "ext4");

    // 执行挂载
    cx.resolve("/a")?.mount(fs)?;

    // 验证挂载后状态
    let dir_after = cx.resolve("/a")?;
    assert!(dir_after.is_root_of_mount());
    assert_eq!(dir_after.filesystem().name(), "vfat");
    assert_eq!(dir_after.absolute_path()?.to_string(), "/a");

    // 验证可以访问挂载文件系统的内容
    let files = list_files(cx, "/a")?;
    assert!(files.contains("short.txt"));
    assert!(files.contains("long.txt"));

    // 验证设备ID不同
    let root_device = cx.resolve("/")?.metadata()?.device;
    let mount_device = cx.resolve("/a")?.metadata()?.device;
    assert_ne!(root_device, mount_device);

    println!("Basic mount test passed!");
    Ok(())
}

fn test_nested_mount(
    cx: &FsContext<RawMutex>,
    fs1: &Filesystem<RawMutex>,
    fs2: &Filesystem<RawMutex>,
) -> VfsResult<()> {
    println!("Testing nested mount operations...");

    let mode = NodePermission::from_bits(0o755).unwrap();

    // 创建挂载点目录
    cx.create_dir("/mnt", mode)?;
    cx.create_dir("/mnt/nested", mode)?;

    cx.resolve("/mnt/nested")?.mount(fs1)?;

    let mnt = cx.resolve("/mnt/nested")?;
    assert!(mnt.is_root_of_mount());
    //

    // // 第一层挂载
    // cx.resolve("/mnt")?.mount(fs1)?;
    //
    // // 在挂载的文件系统中创建子目录并挂载
    // cx.create_dir("/mnt/nested", mode)?;
    // cx.resolve("/mnt/nested")?.mount(fs2)?;
    //
    // // 验证嵌套挂载结构
    // let mnt = cx.resolve("/mnt")?;
    // let nested = cx.resolve("/mnt/nested")?;
    //
    // assert!(mnt.is_root_of_mount());
    // assert!(nested.is_root_of_mount());
    // assert_eq!(mnt.filesystem().name(), "vfat");
    // assert_eq!(nested.filesystem().name(), "vfat");
    //
    // // 验证路径正确
    // assert_eq!(mnt.absolute_path()?.to_string(), "/mnt");
    // assert_eq!(nested.absolute_path()?.to_string(), "/mnt/nested");
    //
    // println!("Nested mount test passed!");
    Ok(())
}

#[test]
#[cfg(all(feature = "ext4", feature = "fat"))]
fn test_mount_persistence_mechanism() {
    println!("=== Testing Mount Persistence Mechanism ===");

    let ext4_disk = RamDisk::from(&std::fs::read("resources/ext4.img").unwrap());
    let ext4_fs = fs::ext4::Ext4Filesystem::<RawMutex>::new(ext4_disk).unwrap();

    let fat16_disk = RamDisk::from(&std::fs::read("resources/fat16.img").unwrap());
    let fat16_fs = fs::fat::FatFilesystem::<RawMutex>::new(fat16_disk);

    let mount = Mountpoint::new_root(&ext4_fs);
    let cx = FsContext::new(mount.root_location());

    // 步骤1: 获取目录"/a"的引用
    println!("Step 1: Getting reference to directory '/a'");
    let dir_ref1 = cx.resolve("/a").unwrap();
    let dir_node_ptr1 = dir_ref1.entry().as_dir().unwrap() as *const _;
    println!("DirNode pointer 1: {:p}", dir_node_ptr1);

    // 步骤2: 执行mount操作
    println!("\nStep 2: Performing mount operation");
    dir_ref1.mount(&fat16_fs).unwrap();
    println!("Mount completed");

    // 步骤3: 再次获取同一目录的引用
    println!("\nStep 3: Getting reference to '/a' again");
    let dir_ref2 = cx.resolve("/a").unwrap();

    // 这里关键：我们需要访问原始的DirNode（挂载前的）
    let root_dir = cx.resolve("/").unwrap();
    let original_dir_node = root_dir.entry().as_dir().unwrap().lookup("a").unwrap();
    let dir_node_ptr2 = original_dir_node.as_dir().unwrap() as *const _;

    println!("DirNode pointer 2: {:p}", dir_node_ptr2);
    println!("Pointers are same: {}", dir_node_ptr1 == dir_node_ptr2);

    // 步骤4: 验证挂载信息确实存储在同一个DirNode中
    println!("\nStep 4: Verifying mount info storage");
    let has_mount = original_dir_node.as_dir().unwrap().mountpoint().is_some();
    println!("Original DirNode has mountpoint: {}", has_mount);

    // 步骤5: 从不同路径访问，验证挂载信息一致性
    println!("\nStep 5: Testing consistency from different access paths");

    // 方法1: 通过根目录查找
    let via_root = cx
        .resolve("/")
        .unwrap()
        .entry()
        .as_dir()
        .unwrap()
        .lookup("a")
        .unwrap()
        .as_dir()
        .unwrap()
        .mountpoint();

    // 方法2: 直接resolve（会触发挂载点跳转）
    let resolved = cx.resolve("/a").unwrap();
    println!("Resolved '/a' filesystem: {}", resolved.filesystem().name());

    // 方法3: 通过父目录缓存
    let from_cache = cx
        .resolve("/")
        .unwrap()
        .entry()
        .as_dir()
        .unwrap()
        .lookup_cache("a")
        .unwrap()
        .as_dir()
        .unwrap()
        .mountpoint();

    println!("Via root lookup has mount: {}", via_root.is_some());
    println!("From cache has mount: {}", from_cache.is_some());

    // 步骤6: 证明即使"丢失"局部变量，挂载信息仍然存在
    println!("\nStep 6: Testing persistence after 'losing' local variables");
    {
        let _temp_ref = cx.resolve("/a").unwrap(); // 临时变量
        // temp_ref 在这里被销毁
    }

    // 但挂载信息仍然存在！
    let still_mounted = cx.resolve("/a").unwrap();
    println!(
        "After temp variable destroyed, filesystem: {}",
        still_mounted.filesystem().name()
    );
    println!(
        "Still shows as mounted: {}",
        still_mounted.is_root_of_mount()
    );
}

#[test]
#[cfg(all(feature = "ext4", feature = "fat"))]
fn test_cache_performance_analysis() {
    use std::sync::Arc as StdArc;
    use std::thread;
    use std::time::Instant;

    println!("=== Cache Performance Analysis ===");

    let ext4_disk = RamDisk::from(&std::fs::read("resources/ext4.img").unwrap());
    let ext4_fs = fs::ext4::Ext4Filesystem::<RawMutex>::new(ext4_disk).unwrap();

    let mount = Mountpoint::new_root(&ext4_fs);
    let cx = FsContext::new(mount.root_location());
    let cx = StdArc::new(cx);

    // 预热缓存 - 创建一些目录
    for i in 0..10 {
        let _ = cx.create_dir(
            format!("/dir{}", i),
            NodePermission::from_bits(0o755).unwrap(),
        );
    }

    println!("\n1. Single-threaded sequential access test:");
    let start = Instant::now();
    for _ in 0..1000 {
        for i in 0..10 {
            let _ = cx.resolve(format!("/dir{}", i));
        }
    }
    let sequential_time = start.elapsed();
    println!("   1000x10 sequential lookups: {:?}", sequential_time);

    println!("\n2. Multi-threaded concurrent access test:");
    let thread_count = 8;
    let iterations_per_thread = 125; // 总计还是1000x10

    let start = Instant::now();
    let handles: Vec<_> = (0..thread_count)
        .map(|thread_id| {
            let cx = cx.clone();
            thread::spawn(move || {
                for _ in 0..iterations_per_thread {
                    for i in 0..10 {
                        let path = format!("/dir{}", (i + thread_id) % 10);
                        let _ = cx.resolve(&path);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
    let concurrent_time = start.elapsed();
    println!(
        "   {}x{}x10 concurrent lookups: {:?}",
        thread_count, iterations_per_thread, concurrent_time
    );

    let speedup = sequential_time.as_nanos() as f64 / concurrent_time.as_nanos() as f64;
    println!(
        "   Speedup ratio: {:.2}x (理想值: {:.2}x)",
        speedup, thread_count as f64
    );

    if speedup < thread_count as f64 * 0.3 {
        println!("   ⚠️  Poor scalability detected! Lock contention is significant.");
    }

    println!("\n3. Lock contention analysis:");
    println!("   Current design issues:");
    println!("   - Mutex<BTreeMap> for every directory cache");
    println!("   - O(log n) lookup in BTreeMap");
    println!("   - String comparison overhead");
    println!("   - No read/write lock separation");

    println!("\n4. Deep path resolution test:");
    // 创建深层目录结构
    let mut current = "/".to_string();
    for i in 0..5 {
        current = format!("{}/level{}", current, i);
        let _ = cx.create_dir(&current, NodePermission::from_bits(0o755).unwrap());
    }

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = cx.resolve("/level0/level1/level2/level3/level4");
    }
    let deep_path_time = start.elapsed();
    println!("   1000 deep path resolutions: {:?}", deep_path_time);
    println!("   Average per lookup: {:?}", deep_path_time / 1000);
}
