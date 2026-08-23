// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use xerrno::LinuxResult;

/// Raw persistent storage behind one coherent file mapping.
///
/// Implementations must bypass the page cache and must not re-enter the same
/// mapping from any method.
pub trait Backing: Send + Sync {
    fn byte_len(&self) -> LinuxResult<u64>;
    fn read_at(&self, offset: u64, destination: &mut [u8]) -> LinuxResult<usize>;
    fn write_at(&self, offset: u64, source: &[u8]) -> LinuxResult<usize>;
    fn set_len(&self, len: u64) -> LinuxResult<()>;
    fn sync(&self, data_only: bool) -> LinuxResult<()>;
}
