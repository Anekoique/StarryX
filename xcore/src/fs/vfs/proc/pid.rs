use alloc::{
    borrow::Cow,
    string::ToString,
    sync::{Arc, Weak},
};

use axfs_ng_vfs::VfsResult;
use axtask::{TaskExtRef, TaskInner, get_task_by_id};

use xutils::collections::weak_map::StrongRef;

use super::DUMMY_MAPS;
use crate::{
    fs::{
        fd::FD_TABLE,
        file::XFile,
        vfs::{
            DirMaker, VirtDir, VirtDirBuilder, VirtDirOps, VirtFile, VirtFileOps, VirtFs,
            VirtNodeOps,
        },
    },
    task::{XProcess, processes},
};

pub(crate) struct ProcPidOps(pub(crate) Arc<VirtFs>);

impl VirtDirOps for ProcPidOps {
    fn read_dir(&self) -> impl Iterator<Item = Cow<'_, str>> {
        processes()
            .into_iter()
            .map(|proc| Cow::Owned(proc.pid().to_string()))
    }

    fn lookup(&self, name: &str) -> Option<VirtNodeOps> {
        let tid = name.parse::<u32>().ok()?;
        get_task_by_id(tid.into())
            .and_then(|weak| weak.upgrade())
            .map(|task| VirtNodeOps::Dir(create_tid_root(self.0.clone(), task.inner())))
    }
}

struct FdOps(Weak<XFile>);

// FIXME: /proc/pid/fd shouldn't be a RegularFile.
impl VirtFileOps for FdOps {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        self.0.upgrade().unwrap().read(buf)
    }
    fn write_at(&self, data: &[u8], _offset: u64) -> VfsResult<usize> {
        self.0.upgrade().unwrap().write(data)
    }
    fn len(&self) -> VfsResult<u64> {
        self.0.upgrade().unwrap().len()
    }
}

fn create_tid_root(fs: Arc<VirtFs>, task: &TaskInner) -> DirMaker {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);
    let thread = task.task_ext().thread();
    let xproc = XProcess::from_thread_static(&thread);
    let fd_root = create_fd_root(fs.clone(), xproc);
    let status = crate::task::status(task);
    let stat = crate::task::stat(task);

    root.add(
        "exe",
        VirtFile::new_symlink(fs.clone(), move || xproc.exe_path.read().to_string()),
    )
    .add("maps", VirtFile::new(fs.clone(), || DUMMY_MAPS.to_string()))
    .add("fd", fd_root.build())
    .add(
        "status",
        VirtFile::new(fs.clone(), move || status.to_string()),
    )
    .add("stat", VirtFile::new(fs.clone(), move || stat.to_string()));

    root.build()
}

fn create_fd_root(fs: Arc<VirtFs>, xproc: &XProcess) -> VirtDirBuilder<()> {
    let fd_table = FD_TABLE.deref_from(&xproc.ns);
    let mut root = VirtDir::<()>::builder(fs.clone(), None);

    for fd in fd_table.ids() {
        root.add(
            fd.to_string(),
            VirtFile::new(
                fs.clone(),
                FdOps(fd_table.get(fd as _).unwrap().downgrade()),
            ),
        );
    }
    root
}
