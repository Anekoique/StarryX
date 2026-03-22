use core::{any::Any, borrow::Borrow, cmp::Ordering, time::Duration};

use alloc::{
    borrow::ToOwned, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec,
};
use axfs_ng_vfs::*;
use axsync::{Mutex, RawMutex};

use xutils::collections::slab::Slab;

use super::virt_fs::dummy_stat;

/// Initialize and return a new temporary filesystem instance
pub fn init_tmpfs() -> Filesystem<RawMutex> {
    MemoryFs::new()
}

#[derive(PartialEq, Eq, Clone)]
struct FileName(String);

impl PartialOrd for FileName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileName {
    fn cmp(&self, other: &Self) -> Ordering {
        let priority = |s: &str| match s {
            "." => 0,
            ".." => 1,
            _ => 2,
        };
        (priority(&self.0), &self.0).cmp(&(priority(&other.0), &other.0))
    }
}

impl<T: Into<String>> From<T> for FileName {
    fn from(name: T) -> Self {
        Self(name.into())
    }
}

impl Borrow<str> for FileName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

pub struct MemoryFs {
    inodes: Mutex<Slab<Arc<MemoryInode>>>,
    root: Mutex<Option<DirEntry<RawMutex>>>,
}

impl MemoryFs {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Filesystem<RawMutex> {
        let fs = Arc::new(Self {
            inodes: Mutex::new(Slab::new()),
            root: Mutex::default(),
        });

        let root_ino = MemoryInode::new(
            &fs,
            None,
            NodeType::Directory,
            NodePermission::from_bits_truncate(0o755),
        );
        *fs.root.lock() = Some(DirEntry::new_dir(
            |this| DirNode::new(MemoryNode::new(fs.clone(), root_ino, Some(this))),
            Reference::root(),
        ));

        Filesystem::new(fs)
    }

    fn get(&self, ino: u64) -> Arc<MemoryInode> {
        self.inodes.lock()[ino as usize - 1].clone()
    }
}

impl FilesystemOps<RawMutex> for MemoryFs {
    fn name(&self) -> &str {
        "tmpfs"
    }

    fn root_dir(&self) -> DirEntry<RawMutex> {
        self.root.lock().clone().unwrap()
    }

    fn stat(&self) -> VfsResult<StatFs> {
        Ok(dummy_stat(0x01021994))
    }
}

fn release_inode(fs: &MemoryFs, inode: &Arc<MemoryInode>, nlink: u64) {
    let mut inodes = fs.inodes.lock();
    let mut metadata = inode.metadata.lock();
    metadata.nlink -= nlink;
    if metadata.nlink == 0 && Arc::strong_count(inode) == 2 {
        debug!("release_inode: {:?}", inode.ino);
        inodes.remove(metadata.inode as usize - 1);
    }
}

type FileContent = Mutex<Vec<u8>>;
type DirContent = Mutex<BTreeMap<FileName, InodeRef>>;

enum NodeContent {
    File(FileContent),
    Dir(DirContent),
}

impl Default for NodeContent {
    fn default() -> Self {
        Self::File(Mutex::default())
    }
}

struct MemoryInode {
    ino: u64,
    metadata: Mutex<Metadata>,
    content: NodeContent,
}

impl MemoryInode {
    pub fn new(
        fs: &Arc<MemoryFs>,
        parent: Option<u64>,
        node_type: NodeType,
        permission: NodePermission,
    ) -> Arc<MemoryInode> {
        let mut inodes = fs.inodes.lock();
        let entry = inodes.vacant_entry();
        let ino = entry.key() as u64 + 1;

        let metadata = Metadata {
            device: 0,
            inode: ino,
            nlink: 0,
            mode: permission,
            node_type,
            uid: 0,
            gid: 0,
            size: 0,
            block_size: 0,
            blocks: 0,
            rdev: DeviceId::default(),
            atime: Duration::default(),
            mtime: Duration::default(),
            ctime: Duration::default(),
        };

        let content = match node_type {
            NodeType::Directory => NodeContent::Dir(Mutex::new(BTreeMap::new())),
            _ => NodeContent::File(Mutex::default()),
        };

        let result = Arc::new(Self {
            ino,
            metadata: Mutex::new(metadata),
            content,
        });

        entry.insert(result.clone());
        drop(inodes);
        if let NodeContent::Dir(entries) = &result.content {
            let mut entries = entries.lock();
            entries.insert(".".into(), InodeRef::new(fs.clone(), ino));
            entries.insert(
                "..".into(),
                InodeRef::new(fs.clone(), parent.unwrap_or(ino)),
            );
        }
        result
    }

    fn as_file(&self) -> VfsResult<&FileContent> {
        match &self.content {
            NodeContent::File(content) => Ok(content),
            _ => Err(VfsError::EISDIR),
        }
    }

    fn as_dir(&self) -> VfsResult<&DirContent> {
        match &self.content {
            NodeContent::Dir(content) => Ok(content),
            _ => Err(VfsError::ENOTDIR),
        }
    }
}

struct InodeRef {
    fs: Arc<MemoryFs>,
    ino: u64,
}

impl InodeRef {
    pub fn new(fs: Arc<MemoryFs>, ino: u64) -> Self {
        fs.get(ino).metadata.lock().nlink += 1;
        Self { fs, ino }
    }

    fn get(&self) -> Arc<MemoryInode> {
        self.fs.get(self.ino)
    }
}

impl Drop for InodeRef {
    fn drop(&mut self) {
        release_inode(&self.fs, &self.get(), 1);
    }
}

struct MemoryNode {
    fs: Arc<MemoryFs>,
    inode: Arc<MemoryInode>,
    this: Option<WeakDirEntry<RawMutex>>,
}

impl MemoryNode {
    pub fn new(
        fs: Arc<MemoryFs>,
        inode: Arc<MemoryInode>,
        this: Option<WeakDirEntry<RawMutex>>,
    ) -> Arc<Self> {
        Arc::new(Self { fs, inode, this })
    }

    fn new_entry(
        &self,
        name: &str,
        node_type: NodeType,
        inode: Arc<MemoryInode>,
    ) -> VfsResult<DirEntry<RawMutex>> {
        let reference = Reference::new(
            self.this.as_ref().and_then(WeakDirEntry::upgrade),
            name.to_owned(),
        );

        Ok(if node_type == NodeType::Directory {
            DirEntry::new_dir(
                |this| DirNode::new(MemoryNode::new(self.fs.clone(), inode, Some(this))),
                reference,
            )
        } else {
            DirEntry::new_file(
                FileNode::new(MemoryNode::new(self.fs.clone(), inode, None)),
                node_type,
                reference,
            )
        })
    }
}

impl NodeOps<RawMutex> for MemoryNode {
    fn inode(&self) -> u64 {
        self.inode.ino
    }

    fn metadata(&self) -> VfsResult<Metadata> {
        let mut metadata = self.inode.metadata.lock().clone();
        metadata.size = match &self.inode.content {
            NodeContent::File(content) => content.lock().len() as u64,
            NodeContent::Dir(entries) => entries.lock().len() as u64,
        };
        Ok(metadata)
    }

    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()> {
        let mut metadata = self.inode.metadata.lock();
        if let Some(mode) = update.mode {
            metadata.mode = mode;
        }
        if let Some((uid, gid)) = update.owner {
            metadata.uid = uid;
            metadata.gid = gid;
        }
        if let Some(atime) = update.atime {
            metadata.atime = atime;
        }
        if let Some(mtime) = update.mtime {
            metadata.mtime = mtime;
        }
        Ok(())
    }

    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex> {
        self.fs.as_ref()
    }

    fn sync(&self, _data_only: bool) -> VfsResult<()> {
        Ok(())
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

impl FileNodeOps<RawMutex> for MemoryNode {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let content = self.inode.as_file()?.lock();
        if offset >= content.len() as u64 {
            return Ok(0);
        }

        let start = offset as usize;
        let available = &content[start..];
        let read_len = buf.len().min(available.len());
        buf[..read_len].copy_from_slice(&available[..read_len]);
        Ok(read_len)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let mut content = self.inode.as_file()?.lock();
        let end_pos = offset as usize + buf.len();

        if end_pos > content.len() {
            content.resize(end_pos, 0);
        }

        let start = offset as usize;
        let write_len = buf.len().min(content.len() - start);
        content[start..start + write_len].copy_from_slice(&buf[..write_len]);
        Ok(write_len)
    }

    fn append(&self, buf: &[u8]) -> VfsResult<(usize, u64)> {
        let mut content = self.inode.as_file()?.lock();
        content.extend_from_slice(buf);
        Ok((buf.len(), buf.len() as u64))
    }

    fn set_len(&self, len: u64) -> VfsResult<()> {
        let mut content = self.inode.as_file()?.lock();
        let len = len as usize;

        if len > content.len() {
            content.resize(len, 0);
        } else {
            content.truncate(len);
        }
        Ok(())
    }

    fn set_symlink(&self, target: &str) -> VfsResult<()> {
        *self.inode.as_file()?.lock() = target.as_bytes().to_vec();
        Ok(())
    }
}

impl DirNodeOps<RawMutex> for MemoryNode {
    fn read_dir(&self, offset: u64, sink: &mut dyn DirEntrySink) -> VfsResult<usize> {
        let entries = self.inode.as_dir()?.lock();
        let mut count = 0;

        for (i, (name, entry)) in entries.iter().enumerate().skip(offset as usize) {
            let inode = entry.get();
            if !sink.accept(
                &name.0,
                entry.ino,
                inode.metadata.lock().node_type,
                i as u64 + 1,
            ) {
                break;
            }
            count += 1;
        }

        Ok(count)
    }

    fn lookup(&self, name: &str) -> VfsResult<DirEntry<RawMutex>> {
        let entries = self.inode.as_dir()?.lock();
        let entry = entries.get(name).ok_or(VfsError::ENOENT)?;
        let inode = entry.get();
        let node_type = inode.metadata.lock().node_type;
        self.new_entry(name, node_type, inode)
    }

    fn create(
        &self,
        name: &str,
        node_type: NodeType,
        permission: NodePermission,
    ) -> VfsResult<DirEntry<RawMutex>> {
        let mut entries = self.inode.as_dir()?.lock();

        if entries.contains_key(name) {
            return Err(VfsError::EEXIST);
        }

        let inode = MemoryInode::new(&self.fs, Some(self.inode.ino), node_type, permission);
        entries.insert(name.into(), InodeRef::new(self.fs.clone(), inode.ino));

        self.new_entry(name, node_type, inode)
    }

    fn link(&self, name: &str, target: &DirEntry<RawMutex>) -> VfsResult<DirEntry<RawMutex>> {
        let mut entries = self.inode.as_dir()?.lock();
        let target_node = target.downcast::<Self>()?;

        if entries.contains_key(name) {
            return Err(VfsError::EEXIST);
        }

        let inode = target_node.inode.clone();
        let node_type = target_node.metadata()?.node_type;
        entries.insert(name.into(), InodeRef::new(self.fs.clone(), inode.ino));

        self.new_entry(name, node_type, inode)
    }

    fn unlink(&self, name: &str) -> VfsResult<()> {
        let mut entries = self.inode.as_dir()?.lock();
        let entry = entries.get(name).ok_or(VfsError::ENOENT)?;

        if let NodeContent::Dir(dir_entries) = &entry.get().content
            && dir_entries.lock().len() > 2
        {
            return Err(VfsError::ENOTEMPTY);
        }

        entries.remove(name);
        Ok(())
    }

    fn rename(&self, src_name: &str, dst_dir: &DirNode<RawMutex>, dst_name: &str) -> VfsResult<()> {
        let dst_node = dst_dir.downcast::<Self>()?;

        if let Ok(entry) = dst_dir.lookup(dst_name) {
            let src_entry = self.lookup(src_name)?;
            if entry.inode() == src_entry.inode() {
                return Ok(());
            }
        }

        let mut src_entries = self.inode.as_dir()?.lock();
        let src_entry = src_entries.remove(src_name).ok_or(VfsError::ENOENT)?;

        dst_node
            .inode
            .as_dir()?
            .lock()
            .insert(dst_name.into(), src_entry);
        Ok(())
    }
}

impl Drop for MemoryNode {
    fn drop(&mut self) {
        release_inode(&self.fs, &self.inode, 0);
    }
}
