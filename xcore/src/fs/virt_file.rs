use core::{any::Any, cmp::Ordering};

use alloc::{borrow::Cow, sync::Arc, vec::Vec};
use axfs_ng_vfs::{
    DeviceId, FileNodeOps, FilesystemOps, Metadata, MetadataUpdate, NodeOps, NodePermission,
    NodeType, VfsError, VfsResult,
};
use axsync::RawMutex;
use inherit_methods_macro::inherit_methods;

use super::virt_fs::{VirtFs, VirtNode};

pub trait VirtFileOps: Send + Sync {
    fn read_all(&self) -> VfsResult<Cow<[u8]>>;
    fn write_all(&self, data: &[u8]) -> VfsResult<()>;
}

pub enum VirtFileOperation<'a> {
    Read,
    Write(&'a [u8]),
}

pub struct RwFile<F>(F);
impl<F, R> RwFile<F>
where
    F: Fn(VirtFileOperation) -> VfsResult<Option<R>> + Send + Sync,
    R: Into<Vec<u8>>,
{
    pub fn new(imp: F) -> Self {
        Self(imp)
    }
}

impl<F, R> VirtFileOps for RwFile<F>
where
    F: Fn(VirtFileOperation) -> VfsResult<Option<R>> + Send + Sync,
    R: Into<Vec<u8>>,
{
    fn read_all(&self) -> VfsResult<Cow<[u8]>> {
        (self.0)(VirtFileOperation::Read).map(|it| Cow::Owned(it.unwrap().into()))
    }

    fn write_all(&self, data: &[u8]) -> VfsResult<()> {
        (self.0)(VirtFileOperation::Write(data)).map(|_| ())
    }
}

impl<F, R> VirtFileOps for F
where
    F: Fn() -> R + Send + Sync + 'static,
    R: Into<Vec<u8>>,
{
    fn read_all(&self) -> VfsResult<Cow<[u8]>> {
        Ok(Cow::Owned((self)().into()))
    }

    fn write_all(&self, _data: &[u8]) -> VfsResult<()> {
        Err(VfsError::EBADF)
    }
}

pub struct VirtFile {
    node: VirtNode,
    ops: Arc<dyn VirtFileOps>,
}
impl VirtFile {
    pub fn new(fs: Arc<VirtFs>, ops: impl VirtFileOps + 'static) -> Arc<Self> {
        let node = VirtNode::new(fs, NodeType::RegularFile, NodePermission::default());
        Arc::new(Self {
            node,
            ops: Arc::new(ops),
        })
    }

    pub fn new_symlink(fs: Arc<VirtFs>, ops: impl VirtFileOps + 'static) -> Arc<Self> {
        let node = VirtNode::new(
            fs,
            NodeType::Symlink,
            NodePermission::from_bits_truncate(0o777),
        );
        Arc::new(Self {
            node,
            ops: Arc::new(ops),
        })
    }
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtFile {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn len(&self) -> VfsResult<u64> {
        Ok(self.ops.read_all()?.len() as u64)
    }
}

impl FileNodeOps<RawMutex> for VirtFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let data = self.ops.read_all()?;
        if offset >= data.len() as u64 {
            return Ok(0);
        }
        let data = &data[offset as usize..];
        let read = data.len().min(buf.len());
        buf[..read].copy_from_slice(&data[..read]);
        Ok(read)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        let data = self.ops.read_all()?;
        let mut data = data.to_vec();
    
        let end_pos = offset as usize + buf.len();
        if data.len() < end_pos {
            data.resize(end_pos, 0);
        }
    
        // safe to copy
        data[offset as usize..offset as usize + buf.len()].copy_from_slice(buf);
    
        self.ops.write_all(&data)?;
        Ok(buf.len())
    }
    

    fn append(&self, buf: &[u8]) -> VfsResult<(usize, u64)> {
        let mut data = self.ops.read_all()?.into_owned();
        data.extend_from_slice(buf);
        self.ops.write_all(&data)?;
        Ok((buf.len(), data.len() as u64))
    }

    fn set_len(&self, len: u64) -> VfsResult<()> {
        let data = self.ops.read_all()?;
        match len.cmp(&(data.len() as u64)) {
            Ordering::Less => self.ops.write_all(&data[..len as usize]),
            Ordering::Greater => {
                let mut new_data = data.into_owned();
                new_data.resize(len as usize, 0);
                self.ops.write_all(&new_data)
            }
            Ordering::Equal => Ok(()),
        }
    }

    fn set_symlink(&self, target: &str) -> VfsResult<()> {
        self.ops.write_all(target.as_bytes())
    }
}

pub trait VirtDeviceOps: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;
}

impl<F> VirtDeviceOps for F
where
    F: Fn(&mut [u8], u64) -> VfsResult<usize> + Send + Sync + 'static,
{
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        (self)(buf, offset)
    }

    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Err(VfsError::EBADF)
    }
}

pub struct VirtDevice {
    node: VirtNode,
    ops: Arc<dyn VirtDeviceOps>,
}
impl VirtDevice {
    pub fn new(
        fs: Arc<VirtFs>,
        node_type: NodeType,
        device_id: DeviceId,
        ops: impl VirtDeviceOps + 'static,
    ) -> Arc<Self> {
        let node = VirtNode::new(fs, node_type, NodePermission::default());
        node.metadata.lock().rdev = device_id;
        Arc::new(Self {
            node,
            ops: Arc::new(ops),
        })
    }
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtDevice {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn len(&self) -> VfsResult<u64> {
        Ok(0)
    }
}

impl FileNodeOps<RawMutex> for VirtDevice {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.ops.read_at(buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.ops.write_at(buf, offset)
    }

    fn append(&self, _buf: &[u8]) -> VfsResult<(usize, u64)> {
        Err(VfsError::ENOTTY)
    }

    fn set_len(&self, _len: u64) -> VfsResult<()> {
        Err(VfsError::ENOTTY)
    }

    fn set_symlink(&self, _target: &str) -> VfsResult<()> {
        Err(VfsError::ENOTTY)
    }
}
