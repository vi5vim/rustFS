//! The FUSE glue: a thin translation layer.
//!
//! Every method locks the shared state, calls the matching semantic operation
//! on `FsState` (which lives in `inode.rs`), and turns its `Result` into a
//! FUSE reply. All the real filesystem logic — and its unit tests — lives in
//! `inode.rs`; this file only speaks the kernel's protocol.

use std::ffi::OsStr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use fuser::{
    BsdFileFlags, FileHandle, Filesystem, FopenFlags, Generation, INodeNo, LockOwner, OpenFlags,
    RenameFlags, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyWrite, Request, TimeOrNow, WriteFlags,
};

use crate::inode::FsState;

/// How long the kernel may cache attributes/entries before re-asking us.
const TTL: Duration = Duration::from_secs(1);

/// Resolve a FUSE `TimeOrNow` (from `setattr`) into a concrete `SystemTime`.
fn resolve_time(t: TimeOrNow) -> SystemTime {
    match t {
        TimeOrNow::SpecificTime(st) => st,
        TimeOrNow::Now => SystemTime::now(),
    }
}

/// The filesystem. All state hides behind a `Mutex` so the `&self` FUSE
/// methods can still mutate it.
pub struct MemFs {
    state: Mutex<FsState>,
}

impl MemFs {
    pub fn new() -> Self {
        MemFs {
            state: Mutex::new(FsState::new()),
        }
    }
}

impl Filesystem for MemFs {
    // ---- read path ----

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let st = self.state.lock().unwrap();
        match st.lookup(parent, &name.to_string_lossy()) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(e) => reply.error(e),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let st = self.state.lock().unwrap();
        match st.getattr(ino) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock: Option<LockOwner>,
        reply: ReplyData,
    ) {
        let st = self.state.lock().unwrap();
        match st.read(ino, offset, size) {
            Ok(bytes) => reply.data(&bytes),
            Err(e) => reply.error(e),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let st = self.state.lock().unwrap();
        let listing = match st.readdir(ino) {
            Ok(l) => l,
            Err(e) => {
                reply.error(e);
                return;
            }
        };
        // `offset` is the index to resume from; the value handed back is the *next* index.
        for (i, (e_ino, kind, name)) in listing.iter().enumerate().skip(offset as usize) {
            if reply.add(*e_ino, (i + 1) as u64, *kind, name) {
                break; // kernel buffer full; it will call again with a higher offset.
            }
        }
        reply.ok();
    }

    // ---- write path ----

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let mut st = self.state.lock().unwrap();
        match st.create(parent, &name.to_string_lossy(), (mode & 0o7777) as u16) {
            // FileHandle(0): we don't track per-open handles yet.
            Ok(attr) => reply.created(&TTL, &attr, Generation(0), FileHandle(0), FopenFlags::empty()),
            Err(e) => reply.error(e),
        }
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        let mut st = self.state.lock().unwrap();
        match st.write(ino, offset, data) {
            Ok(n) => reply.written(n),
            Err(e) => reply.error(e),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        let mut st = self.state.lock().unwrap();
        let perm = mode.map(|m| (m & 0o7777) as u16);
        match st.setattr(ino, size, perm, atime.map(resolve_time), mtime.map(resolve_time)) {
            Ok(attr) => reply.attr(&TTL, &attr),
            Err(e) => reply.error(e),
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let mut st = self.state.lock().unwrap();
        match st.mkdir(parent, &name.to_string_lossy(), (mode & 0o7777) as u16) {
            Ok(attr) => reply.entry(&TTL, &attr, Generation(0)),
            Err(e) => reply.error(e),
        }
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let mut st = self.state.lock().unwrap();
        match st.unlink(parent, &name.to_string_lossy()) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let mut st = self.state.lock().unwrap();
        match st.rmdir(parent, &name.to_string_lossy()) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        _flags: RenameFlags,
        reply: ReplyEmpty,
    ) {
        let mut st = self.state.lock().unwrap();
        match st.rename(
            parent,
            &name.to_string_lossy(),
            newparent,
            &newname.to_string_lossy(),
        ) {
            Ok(()) => reply.ok(),
            Err(e) => reply.error(e),
        }
    }
}
