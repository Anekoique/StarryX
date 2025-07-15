use axfs_ng::OpenOptions;

use super::{
    __kernel_mode_t, O_APPEND, O_CLOEXEC, O_CREAT, O_DIRECT, O_DIRECTORY, O_EXCL, O_EXEC, O_RDONLY,
    O_TRUNC, O_WRONLY, c_int,
};

/// Convert open flags to [`OpenOptions`].
pub fn flags_to_options(
    flags: c_int,
    mode: __kernel_mode_t,
    (uid, gid): (u32, u32),
) -> OpenOptions {
    let flags = flags as u32;
    let mut options = OpenOptions::new();
    options.mode(mode).user(uid, gid);
    match flags & 0b11 {
        O_RDONLY => options.read(true),
        O_WRONLY => options.write(true),
        _ => options.read(true).write(true),
    };
    if flags & O_APPEND != 0 {
        options.append(true);
    }
    if flags & O_TRUNC != 0 {
        options.truncate(true);
    }
    if flags & O_CREAT != 0 {
        options.create(true);
    }
    if flags & O_EXEC != 0 {
        options.execute(true);
    }
    if flags & O_EXCL != 0 {
        options.create_new(true);
    }
    if flags & O_DIRECTORY != 0 {
        options.directory(true);
    }
    if flags & O_CLOEXEC != 0 {
        options.cloexec(true);
    }
    if flags & O_DIRECT != 0 {
        options.direct(true);
    }
    options
}
