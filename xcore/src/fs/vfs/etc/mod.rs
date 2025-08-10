use alloc::{string::ToString, sync::Arc};

use axfs_ng_vfs::Filesystem;
use axsync::RawMutex;

use super::{
    virt_file::{DirMaker, VirtDir, VirtFile},
    virt_fs::VirtFs,
};

const PASSWD_CONTENT: &str = concat!(
    "root:x:0:0:root:/root:/bin/bash\n",
    "nobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n",
);
const PROTOCOLS_CONTENT: &str = concat!(
    "ip      0       IP\n",
    "icmp    1       ICMP\n",
    "tcp     6       TCP\n",
    "udp     17      UDP\n",
);

// Minimal termcap entries for common terminals (vt100/xterm/linux/dumb)
// Enough for basic line editing and clear/positioning.
const TERMCAP_CONTENT: &str = r#"vt100|vt100-am|dec vt100:\
    :am:bs:mi:ms:xn:\
    :co#80:li#24:\
    :AL=\E[L:DL=\E[M:al=\E[L:dl=\E[M:\
    :cl=\E[H\E[2J:sf=\ED:sr=\EM:\
    :cm=\E[%i%p1%d;%p2%dH:nd=\E[C:up=\E[A:le=\E[D:\
    :so=\E[7m:se=\E[27m:us=\E[4m:ue=\E[24m:md=\E[1m:me=\E[0m:\
    :ce=\E[K:cd=\E[J:\
    :ti=\E[?1049h:te=\E[?1049l:\
    :ks=\E[?1h\E=:ke=\E[?1l\E=:\
    :kb=\177:ku=\E[A:kd=\E[B:kr=\E[C:kl=\E[D:

xterm|xterm-color:\
    :am:bs:mi:ms:xn:\
    :co#80:li#24:\
    :cl=\E[H\E[2J:cm=\E[%i%p1%d;%p2%dH:nd=\E[C:up=\E[A:le=\E[D:\
    :so=\E[7m:se=\E[27m:us=\E[4m:ue=\E[24m:md=\E[1m:me=\E[0m:\
    :ce=\E[K:cd=\E[J:\
    :ks=\E[?1h\E=:ke=\E[?1l\E=:\
    :ti=\E[?1049h:te=\E[?1049l:\
    :kb=\177:ku=\E[A:kd=\E[B:kr=\E[C:kl=\E[D:

linux|linux console:\
    :am:bs:co#80:li#25:\
    :cl=\E[H\E[J:cm=\E[%i%p1%d;%p2%dH:nd=\E[C:up=\E[A:le=\E[D:\
    :so=\E[7m:se=\E[27m:us=\E[4m:ue=\E[24m:md=\E[1m:me=\E[0m:\
    :ce=\E[K:cd=\E[J:\
    :kb=\177:ku=\E[A:kd=\E[B:kr=\E[C:kl=\E[D:

dumb|80-column dumb tty:\
    :co#80:li#24:\
    :cl=^L:le=^H:bs:
"#;

const INPUTRC_CONTENT: &str = r#"# do not bell on tab-completion
#set bell-style none

set meta-flag on
set input-meta on
set convert-meta off
set output-meta on

$if mode=emacs

# for linux console and RH/Debian xterm
"\e[1~": beginning-of-line
"\e[4~": end-of-line
"\e[5~": beginning-of-history
"\e[6~": end-of-history
"\e[7~": beginning-of-line
"\e[3~": delete-char
"\e[2~": quoted-insert
"\e[5C": forward-word
"\e[5D": backward-word
"\e\e[C": forward-word
"\e\e[D": backward-word
"\e[1;5C": forward-word
"\e[1;5D": backward-word

# for rxvt
"\e[8~": end-of-line

# for non RH/Debian xterm, can't hurt for RH/DEbian xterm
"\eOH": beginning-of-line
"\eOF": end-of-line

# for freebsd console
"\e[H": beginning-of-line
"\e[F": end-of-line
$endif
"#;

/// Initialize the /etc filesystem as a virtual filesystem.
pub fn init_etcfs() -> Filesystem<RawMutex> {
    VirtFs::new_with("etcfs".into(), 0x657463, create_etc_root) // magic number for 'etc'
}

/// Create the root /etc directory structure.
fn create_etc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);
    root.add(
        "passwd",
        VirtFile::new(fs.clone(), || PASSWD_CONTENT.to_string()),
    )
    .add(
        "protocols",
        VirtFile::new(fs.clone(), || PROTOCOLS_CONTENT.to_string()),
    )
    .add(
        "termcap",
        VirtFile::new(fs.clone(), || TERMCAP_CONTENT.to_string()),
    )
    .add(
        "inputrc",
        VirtFile::new(fs.clone(), || INPUTRC_CONTENT.to_string()),
    );
    root.build()
}
