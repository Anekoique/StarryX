#![cfg_attr(not(test), no_std)]
#![feature(variant_count)]
#![doc = include_str!("../README.md")]

use core::fmt;

mod linux_errno {
    include!(concat!(env!("OUT_DIR"), "/linux_errno.rs"));
}

pub use linux_errno::LinuxError;

/// The error type used by ArceOS.
///
/// Similar to [`std::io::ErrorKind`].
///
/// [`std::io::ErrorKind`]: https://doc.rust-lang.org/std/io/enum.ErrorKind.html
#[repr(i32)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XError {
    /// A socket address could not be bound because the address is already in use elsewhere.
    AddrInUse = 1,
    /// An entity already exists, often a file.
    AlreadyExists,
    /// Bad address.
    BadAddress,
    /// Bad internal state.
    BadState,
    /// The connection was refused by the remote server,
    ConnectionRefused,
    /// The connection was reset by the remote server.
    ConnectionReset,
    /// A non-empty directory was specified where an empty directory was expected.
    DirectoryNotEmpty,
    /// Data not valid for the operation were encountered.
    ///
    /// Unlike [`InvalidInput`], this typically means that the operation
    /// parameters were valid, however the error was caused by malformed
    /// input data.
    ///
    /// For example, a function that reads a file into a string will error with
    /// `InvalidData` if the file's contents are not valid UTF-8.
    ///
    /// [`InvalidInput`]: XError::InvalidInput
    InvalidData,
    /// Invalid parameter/argument.
    InvalidInput,
    /// Input/output error.
    Io,
    /// The filesystem object is, unexpectedly, a directory.
    IsADirectory,
    /// Not enough space/cannot allocate memory.
    NoMemory,
    /// A filesystem object is, unexpectedly, not a directory.
    NotADirectory,
    /// The network operation failed because it was not connected yet.
    NotConnected,
    /// The requested entity is not found.
    NotFound,
    /// The operation lacked the necessary privileges to complete.
    PermissionDenied,
    /// Device or resource is busy.
    ResourceBusy,
    /// The underlying storage (typically, a filesystem) is full.
    StorageFull,
    /// An error returned when an operation could not be completed because an
    /// "end of file" was reached prematurely.
    UnexpectedEof,
    /// This operation is unsupported or unimplemented.
    Unsupported,
    /// The operation needs to block to complete, but the blocking operation was
    /// requested to not occur.
    WouldBlock,
    /// An error returned when an operation could not be completed because a
    /// call to `write()` returned [`Ok(0)`](Ok).
    WriteZero,
}

/// A specialized [`Result`] type with [`XError`] as the error type.
pub type XResult<T = ()> = Result<T, XError>;

/// A specialized [`Result`] type with [`LinuxError`] as the error type.
pub type LinuxResult<T = ()> = Result<T, LinuxError>;

/// Convenience method to construct an [`XError`] type while printing a warning
/// message.
///
/// # Examples
///
/// ```
/// # use xerrno::{x_err_type, XError};
/// #
/// // Also print "[XError::AlreadyExists]" if the `log` crate is enabled.
/// assert_eq!(
///     x_err_type!(AlreadyExists),
///     XError::AlreadyExists,
/// );
///
/// // Also print "[XError::BadAddress] the address is 0!" if the `log` crate
/// // is enabled.
/// assert_eq!(
///     x_err_type!(BadAddress, "the address is 0!"),
///     XError::BadAddress,
/// );
/// ```
#[macro_export]
macro_rules! x_err_type {
    ($err: ident) => {{
        use $crate::XError::*;
        $crate::__priv::warn!("[XError::{:?}]", $err);
        $err
    }};
    ($err: ident, $msg: expr) => {{
        use $crate::XError::*;
        $crate::__priv::warn!("[XError::{:?}] {}", $err, $msg);
        $err
    }};
}

/// Ensure a condition is true. If it is not, return from the function
/// with an error.
///
/// ## Examples
///
/// ```rust
/// # use xerrno::{ensure, x_err, XError, XResult};
///
/// fn example(user_id: i32) -> XResult {
///     ensure!(user_id > 0, x_err!(InvalidInput));
///     // After this point, we know that `user_id` is positive.
///     let user_id = user_id as u32;
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! ensure {
    ($predicate:expr, $context_selector:expr $(,)?) => {
        if !$predicate {
            return $context_selector;
        }
    };
}

/// Convenience method to construct an [`Err(XError)`] type while printing a
/// warning message.
///
/// # Examples
///
/// ```
/// # use xerrno::{x_err, XResult, XError};
/// #
/// // Also print "[XError::AlreadyExists]" if the `log` crate is enabled.
/// assert_eq!(
///     x_err!(AlreadyExists),
///     XResult::<()>::Err(XError::AlreadyExists),
/// );
///
/// // Also print "[XError::BadAddress] the address is 0!" if the `log` crate is enabled.
/// assert_eq!(
///     x_err!(BadAddress, "the address is 0!"),
///     XResult::<()>::Err(XError::BadAddress),
/// );
/// ```
/// [`Err(XError)`]: Err
#[macro_export]
macro_rules! x_err {
    ($err: ident) => {
        Err($crate::x_err_type!($err))
    };
    ($err: ident, $msg: expr) => {
        Err($crate::x_err_type!($err, $msg))
    };
}

impl XError {
    /// Returns the error description.
    pub fn as_str(&self) -> &'static str {
        use XError::*;
        match *self {
            AddrInUse => "Address in use",
            BadAddress => "Bad address",
            BadState => "Bad internal state",
            AlreadyExists => "Entity already exists",
            ConnectionRefused => "Connection refused",
            ConnectionReset => "Connection reset",
            DirectoryNotEmpty => "Directory not empty",
            InvalidData => "Invalid data",
            InvalidInput => "Invalid input parameter",
            Io => "I/O error",
            IsADirectory => "Is a directory",
            NoMemory => "Out of memory",
            NotADirectory => "Not a directory",
            NotConnected => "Not connected",
            NotFound => "Entity not found",
            PermissionDenied => "Permission denied",
            ResourceBusy => "Resource busy",
            StorageFull => "No storage space",
            UnexpectedEof => "Unexpected end of file",
            Unsupported => "Operation not supported",
            WouldBlock => "Operation would block",
            WriteZero => "Write zero",
        }
    }

    /// Returns the error code value in `i32`.
    pub const fn code(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for XError {
    type Error = i32;

    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value > 0 && value <= core::mem::variant_count::<XError>() as i32 {
            Ok(unsafe { core::mem::transmute::<i32, XError>(value) })
        } else {
            Err(value)
        }
    }
}

impl fmt::Display for XError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<XError> for LinuxError {
    fn from(e: XError) -> Self {
        use XError::*;
        match e {
            AddrInUse => LinuxError::EADDRINUSE,
            AlreadyExists => LinuxError::EEXIST,
            BadAddress | BadState => LinuxError::EFAULT,
            ConnectionRefused => LinuxError::ECONNREFUSED,
            ConnectionReset => LinuxError::ECONNRESET,
            DirectoryNotEmpty => LinuxError::ENOTEMPTY,
            InvalidInput | InvalidData => LinuxError::EINVAL,
            Io => LinuxError::EIO,
            IsADirectory => LinuxError::EISDIR,
            NoMemory => LinuxError::ENOMEM,
            NotADirectory => LinuxError::ENOTDIR,
            NotConnected => LinuxError::ENOTCONN,
            NotFound => LinuxError::ENOENT,
            PermissionDenied => LinuxError::EACCES,
            ResourceBusy => LinuxError::EBUSY,
            StorageFull => LinuxError::ENOSPC,
            Unsupported => LinuxError::ENOSYS,
            UnexpectedEof | WriteZero => LinuxError::EIO,
            WouldBlock => LinuxError::EAGAIN,
        }
    }
}

impl fmt::Display for LinuxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[doc(hidden)]
pub mod __priv {
    pub use log::warn;
}

#[cfg(test)]
mod tests {
    use crate::XError;

    #[test]
    fn test_try_from() {
        let max_code = core::mem::variant_count::<XError>() as i32;
        assert_eq!(max_code, 22);
        assert_eq!(max_code, XError::WriteZero.code());

        assert_eq!(XError::AddrInUse.code(), 1);
        assert_eq!(Ok(XError::AddrInUse), XError::try_from(1));
        assert_eq!(Ok(XError::AlreadyExists), XError::try_from(2));
        assert_eq!(Ok(XError::WriteZero), XError::try_from(max_code));
        assert_eq!(Err(max_code + 1), XError::try_from(max_code + 1));
        assert_eq!(Err(0), XError::try_from(0));
        assert_eq!(Err(-1), XError::try_from(-1));
        assert_eq!(Err(i32::MAX), XError::try_from(i32::MAX));
    }
}
