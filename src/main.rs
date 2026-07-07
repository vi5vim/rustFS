//! rustFS — a user-level filesystem built on FUSE.
//!
//! This is a minimal, read-only starting point: it exposes a single file,
//! `hello.txt`, at the mount root. It exists to prove the toolchain works
//! end-to-end (macFUSE -> fuser -> kernel -> `ls`/`cat`) and to give you a
//! skeleton whose methods you can flesh out as you build a real FS.
//!
//! Run with:  cargo run -- <mountpoint>
//! Unmount with:  umount <mountpoint>   (macOS)  or  fusermount -u <mountpoint> (Linux)

use std::env;
use std::ffi::OsStr;
use std::time::{Duration, UNIX_EPOCH};

use fuser::{
    Config, Errno, FileAttr, FileHandle, FileType, Filesystem, Generation, INodeNo, LockOwner,
    MountOption, OpenFlags, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};

/// How long the kernel may cache attributes/entries before asking us again.
const TTL: Duration = Duration::from_secs(1);

/// Inode number for the single `hello.txt` file. (Inode 1 is the root, `INodeNo::ROOT`.)
const HELLO_INO: INodeNo = INodeNo(2);

const HELLO_NAME: &str = "hello.txt";
const HELLO_CONTENT: &str = "Hello from rustFS!\n";

/// Attributes for the root directory.
fn root_attr() -> FileAttr {
    FileAttr {
        ino: INodeNo::ROOT,
        size: 0,
        blocks: 0,
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::Directory,
        perm: 0o755,
        nlink: 2,
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
        rdev: 0,
        flags: 0,
        blksize: 512,
    }
}

/// Attributes for the single `hello.txt` file.
fn hello_attr() -> FileAttr {
    FileAttr {
        ino: HELLO_INO,
        size: HELLO_CONTENT.len() as u64,
        blocks: 1,
        atime: UNIX_EPOCH,
        mtime: UNIX_EPOCH,
        ctime: UNIX_EPOCH,
        crtime: UNIX_EPOCH,
        kind: FileType::RegularFile,
        perm: 0o644,
        nlink: 1,
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
        rdev: 0,
        flags: 0,
        blksize: 512,
    }
}

struct RustFs;

impl Filesystem for RustFs {
    /// Resolve a name within a directory to an inode + attributes.
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        if parent == INodeNo::ROOT && name.to_str() == Some(HELLO_NAME) {
            reply.entry(&TTL, &hello_attr(), Generation(0));
        } else {
            reply.error(Errno::ENOENT);
        }
    }

    /// Return the attributes for a given inode (the FS-level `stat`).
    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        match ino {
            INodeNo::ROOT => reply.attr(&TTL, &root_attr()),
            HELLO_INO => reply.attr(&TTL, &hello_attr()),
            _ => reply.error(Errno::ENOENT),
        }
    }

    /// Read bytes from a file.
    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        _size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        if ino == HELLO_INO {
            let start = (offset as usize).min(HELLO_CONTENT.len());
            reply.data(&HELLO_CONTENT.as_bytes()[start..]);
        } else {
            reply.error(Errno::ENOENT);
        }
    }

    /// List the contents of a directory.
    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        if ino != INodeNo::ROOT {
            reply.error(Errno::ENOENT);
            return;
        }

        // (inode, kind, name) for every entry, including `.` and `..`.
        let entries = [
            (INodeNo::ROOT, FileType::Directory, "."),
            (INodeNo::ROOT, FileType::Directory, ".."),
            (HELLO_INO, FileType::RegularFile, HELLO_NAME),
        ];

        // `offset` lets the kernel resume a partial listing; skip what it already has.
        for (i, (ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            // The offset we hand back is "next entry to read" = i + 1.
            // reply.add returns true when the buffer is full; stop if so.
            if reply.add(*ino, (i + 1) as u64, *kind, name) {
                break;
            }
        }
        reply.ok();
    }
}

fn main() {
    let mountpoint = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: rustFS <mountpoint>");
            std::process::exit(1);
        }
    };

    let mut config = Config::default();
    config.mount_options = vec![
        MountOption::RO,
        MountOption::FSName("rustFS".to_string()),
    ];

    println!("Mounting rustFS at {mountpoint} (Ctrl-C to unmount)...");
    fuser::mount2(RustFs, &mountpoint, &config).expect("failed to mount rustFS");
}
