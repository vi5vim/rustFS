//! In-memory data model & filesystem semantics. FUSE-free: each operation is
//! a plain method returning `Result<_, Errno>`, unit-testable without mounting.

use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;

use fuser::{Errno, FileAttr, FileType, INodeNo};

pub struct Inode {
    pub attr: FileAttr,
    /// Parent directory. Root is its own parent. Answers `..`.
    pub parent: INodeNo,
    pub data: NodeData,
}

pub enum NodeData {
    File(Vec<u8>),
    /// Entry name -> child inode. `.`/`..` synthesized in `readdir`.
    Dir(BTreeMap<String, INodeNo>),
}

pub struct FsState {
    pub inodes: HashMap<INodeNo, Inode>,
    next_ino: u64,
}

impl FsState {
    /// Fresh filesystem: root (inode 1) containing `hello.txt` (inode 2).
    pub fn new() -> Self {
        let now = SystemTime::now();
        let mut inodes = HashMap::new();

        let mut root_entries = BTreeMap::new();
        root_entries.insert("hello.txt".to_string(), INodeNo(2));
        inodes.insert(
            INodeNo::ROOT,
            Inode {
                attr: mk_attr(INodeNo::ROOT, FileType::Directory, 0, 0o755, 2, now),
                parent: INodeNo::ROOT,
                data: NodeData::Dir(root_entries),
            },
        );

        let content = b"Hello from rustFS!\n".to_vec();
        inodes.insert(
            INodeNo(2),
            Inode {
                attr: mk_attr(
                    INodeNo(2),
                    FileType::RegularFile,
                    content.len() as u64,
                    0o644,
                    1,
                    now,
                ),
                parent: INodeNo::ROOT,
                data: NodeData::File(content),
            },
        );

        FsState { inodes, next_ino: 3 }
    }

    fn alloc_ino(&mut self) -> INodeNo {
        let ino = INodeNo(self.next_ino);
        self.next_ino += 1;
        ino
    }

    // ---- read operations ----

    pub fn getattr(&self, ino: INodeNo) -> Result<FileAttr, Errno> {
        self.inodes.get(&ino).map(|i| i.attr).ok_or(Errno::ENOENT)
    }

    pub fn lookup(&self, parent: INodeNo, name: &str) -> Result<FileAttr, Errno> {
        let ino = self.dir_map(parent)?.get(name).copied().ok_or(Errno::ENOENT)?;
        self.getattr(ino)
    }

    /// Listing: `.`, `..`, then each child.
    pub fn readdir(&self, ino: INodeNo) -> Result<Vec<(INodeNo, FileType, String)>, Errno> {
        let inode = self.inodes.get(&ino).ok_or(Errno::ENOENT)?;
        let entries = match &inode.data {
            NodeData::Dir(m) => m,
            NodeData::File(_) => return Err(Errno::ENOTDIR),
        };
        let mut out = vec![
            (ino, FileType::Directory, ".".to_string()),
            (inode.parent, FileType::Directory, "..".to_string()),
        ];
        for (name, child) in entries {
            let kind = self
                .inodes
                .get(child)
                .map(|c| c.attr.kind)
                .unwrap_or(FileType::RegularFile);
            out.push((*child, kind, name.clone()));
        }
        Ok(out)
    }

    pub fn read(&self, ino: INodeNo, offset: u64, size: u32) -> Result<Vec<u8>, Errno> {
        match self.inodes.get(&ino) {
            Some(Inode {
                data: NodeData::File(buf),
                ..
            }) => {
                let start = (offset as usize).min(buf.len());
                let end = start.saturating_add(size as usize).min(buf.len());
                Ok(buf[start..end].to_vec())
            }
            Some(_) => Err(Errno::EISDIR),
            None => Err(Errno::ENOENT),
        }
    }

    // ---- write operations ----

    pub fn create(&mut self, parent: INodeNo, name: &str, perm: u16) -> Result<FileAttr, Errno> {
        self.insert_child(parent, name, NodeData::File(Vec::new()), FileType::RegularFile, perm, 1)
    }

    pub fn mkdir(&mut self, parent: INodeNo, name: &str, perm: u16) -> Result<FileAttr, Errno> {
        self.insert_child(parent, name, NodeData::Dir(BTreeMap::new()), FileType::Directory, perm, 2)
    }

    /// Write `data` at `offset`, growing (and zero-filling gaps in) the file.
    pub fn write(&mut self, ino: INodeNo, offset: u64, data: &[u8]) -> Result<u32, Errno> {
        let inode = self.inodes.get_mut(&ino).ok_or(Errno::ENOENT)?;
        let buf = match &mut inode.data {
            NodeData::File(buf) => buf,
            NodeData::Dir(_) => return Err(Errno::EISDIR),
        };
        let start = offset as usize;
        let end = start + data.len();
        if buf.len() < end {
            buf.resize(end, 0);
        }
        buf[start..end].copy_from_slice(data);

        let new_len = buf.len() as u64;
        inode.attr.size = new_len;
        inode.attr.blocks = new_len.div_ceil(512);
        inode.attr.mtime = SystemTime::now();
        Ok(data.len() as u32)
    }

    /// Apply attribute changes. `size` truncates/extends file contents.
    pub fn setattr(
        &mut self,
        ino: INodeNo,
        size: Option<u64>,
        perm: Option<u16>,
        atime: Option<SystemTime>,
        mtime: Option<SystemTime>,
    ) -> Result<FileAttr, Errno> {
        let inode = self.inodes.get_mut(&ino).ok_or(Errno::ENOENT)?;
        if let Some(sz) = size {
            if let NodeData::File(buf) = &mut inode.data {
                buf.resize(sz as usize, 0);
            }
            inode.attr.size = sz;
            inode.attr.blocks = sz.div_ceil(512);
        }
        if let Some(p) = perm {
            inode.attr.perm = p;
        }
        if let Some(t) = atime {
            inode.attr.atime = t;
        }
        if let Some(t) = mtime {
            inode.attr.mtime = t;
        }
        Ok(inode.attr)
    }

    pub fn unlink(&mut self, parent: INodeNo, name: &str) -> Result<(), Errno> {
        let ino = self.dir_map(parent)?.get(name).copied().ok_or(Errno::ENOENT)?;
        if matches!(
            self.inodes.get(&ino),
            Some(Inode { data: NodeData::Dir(_), .. })
        ) {
            return Err(Errno::EISDIR);
        }
        if let Some(Inode { data: NodeData::Dir(entries), .. }) = self.inodes.get_mut(&parent) {
            entries.remove(name);
        }
        self.inodes.remove(&ino);
        Ok(())
    }

    pub fn rmdir(&mut self, parent: INodeNo, name: &str) -> Result<(), Errno> {
        let ino = self.dir_map(parent)?.get(name).copied().ok_or(Errno::ENOENT)?;
        match self.inodes.get(&ino) {
            Some(Inode { data: NodeData::Dir(entries), .. }) => {
                if !entries.is_empty() {
                    return Err(Errno::ENOTEMPTY);
                }
            }
            Some(_) => return Err(Errno::ENOTDIR),
            None => return Err(Errno::ENOENT),
        }
        // Detach and undo the parent's `..` link bump.
        if let Some(Inode {
            attr: parent_attr,
            data: NodeData::Dir(entries),
            ..
        }) = self.inodes.get_mut(&parent)
        {
            entries.remove(name);
            parent_attr.nlink = parent_attr.nlink.saturating_sub(1);
        }
        self.inodes.remove(&ino);
        Ok(())
    }

    pub fn rename(
        &mut self,
        parent: INodeNo,
        name: &str,
        newparent: INodeNo,
        newname: &str,
    ) -> Result<(), Errno> {
        // Validate destination first so failure never leaves the source half-modified.
        self.dir_map(newparent)?;

        let ino = match self.inodes.get_mut(&parent) {
            Some(Inode { data: NodeData::Dir(entries), .. }) => {
                entries.remove(name).ok_or(Errno::ENOENT)?
            }
            Some(_) => return Err(Errno::ENOTDIR),
            None => return Err(Errno::ENOENT),
        };

        // Attach at destination, capturing any entry it overwrites.
        let replaced = match self.inodes.get_mut(&newparent) {
            Some(Inode { data: NodeData::Dir(entries), .. }) => {
                entries.insert(newname.to_string(), ino)
            }
            _ => None, // unreachable: validated above
        };
        if let Some(old) = replaced {
            self.inodes.remove(&old);
        }

        // Repoint at new parent (keeps `..` correct).
        if let Some(inode) = self.inodes.get_mut(&ino) {
            inode.parent = newparent;
        }
        Ok(())
    }

    // ---- helpers ----

    fn dir_map(&self, ino: INodeNo) -> Result<&BTreeMap<String, INodeNo>, Errno> {
        match self.inodes.get(&ino) {
            Some(Inode { data: NodeData::Dir(m), .. }) => Ok(m),
            Some(_) => Err(Errno::ENOTDIR),
            None => Err(Errno::ENOENT),
        }
    }

    /// Shared body of `create`/`mkdir`.
    fn insert_child(
        &mut self,
        parent: INodeNo,
        name: &str,
        data: NodeData,
        kind: FileType,
        perm: u16,
        nlink: u32,
    ) -> Result<FileAttr, Errno> {
        if self.dir_map(parent)?.contains_key(name) {
            return Err(Errno::EEXIST);
        }
        let is_dir = matches!(data, NodeData::Dir(_));
        let ino = self.alloc_ino();
        let attr = mk_attr(ino, kind, 0, perm, nlink, SystemTime::now());
        self.inodes.insert(ino, Inode { attr, parent, data });

        if let Some(Inode {
            attr: parent_attr,
            data: NodeData::Dir(entries),
            ..
        }) = self.inodes.get_mut(&parent)
        {
            entries.insert(name.to_string(), ino);
            if is_dir {
                parent_attr.nlink += 1; // the new subdir's `..` links back here
            }
        }
        Ok(attr)
    }
}

pub fn mk_attr(
    ino: INodeNo,
    kind: FileType,
    size: u64,
    perm: u16,
    nlink: u32,
    time: SystemTime,
) -> FileAttr {
    FileAttr {
        ino,
        size,
        blocks: size.div_ceil(512),
        atime: time,
        mtime: time,
        ctime: time,
        crtime: time,
        kind,
        perm,
        nlink,
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
        rdev: 0,
        flags: 0,
        blksize: 512,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs() -> FsState {
        FsState::new()
    }

    #[test]
    fn seeded_root_contains_hello() {
        let st = fs();
        let attr = st.lookup(INodeNo::ROOT, "hello.txt").unwrap();
        assert_eq!(attr.kind, FileType::RegularFile);
        assert_eq!(attr.size, 19);
        assert_eq!(st.read(attr.ino, 0, 100).unwrap(), b"Hello from rustFS!\n");
    }

    #[test]
    fn create_write_read_roundtrip() {
        let mut st = fs();
        let attr = st.create(INodeNo::ROOT, "foo", 0o644).unwrap();
        assert_eq!(attr.size, 0);
        assert_eq!(st.write(attr.ino, 0, b"hello").unwrap(), 5);
        assert_eq!(st.read(attr.ino, 0, 100).unwrap(), b"hello");
        assert_eq!(st.getattr(attr.ino).unwrap().size, 5);
    }

    #[test]
    fn duplicate_create_is_eexist() {
        let mut st = fs();
        st.create(INodeNo::ROOT, "foo", 0o644).unwrap();
        let err = st.create(INodeNo::ROOT, "foo", 0o644).unwrap_err();
        assert_eq!(err.code(), libc::EEXIST);
    }

    #[test]
    fn write_past_eof_zero_fills_gap() {
        let mut st = fs();
        let f = st.create(INodeNo::ROOT, "f", 0o644).unwrap();
        st.write(f.ino, 3, b"X").unwrap();
        assert_eq!(st.read(f.ino, 0, 100).unwrap(), vec![0, 0, 0, b'X']);
    }

    #[test]
    fn setattr_truncates() {
        let mut st = fs();
        let f = st.create(INodeNo::ROOT, "f", 0o644).unwrap();
        st.write(f.ino, 0, b"abcdef").unwrap();
        let attr = st.setattr(f.ino, Some(2), None, None, None).unwrap();
        assert_eq!(attr.size, 2);
        assert_eq!(st.read(f.ino, 0, 100).unwrap(), b"ab");
    }

    #[test]
    fn mkdir_bumps_parent_nlink_and_nests() {
        let mut st = fs();
        let d = st.mkdir(INodeNo::ROOT, "sub", 0o755).unwrap();
        assert_eq!(d.kind, FileType::Directory);
        assert_eq!(st.getattr(INodeNo::ROOT).unwrap().nlink, 3); // 2 + new "sub/.."
        let inner = st.create(d.ino, "inner", 0o644).unwrap();
        assert_eq!(st.lookup(d.ino, "inner").unwrap().ino, inner.ino);
    }

    #[test]
    fn unlink_removes_file_but_refuses_dir() {
        let mut st = fs();
        st.create(INodeNo::ROOT, "foo", 0o644).unwrap();
        st.unlink(INodeNo::ROOT, "foo").unwrap();
        assert_eq!(st.lookup(INodeNo::ROOT, "foo").unwrap_err().code(), libc::ENOENT);

        st.mkdir(INodeNo::ROOT, "d", 0o755).unwrap();
        assert_eq!(st.unlink(INodeNo::ROOT, "d").unwrap_err().code(), libc::EISDIR);
    }

    #[test]
    fn rmdir_requires_empty_and_restores_nlink() {
        let mut st = fs();
        let d = st.mkdir(INodeNo::ROOT, "d", 0o755).unwrap();
        st.create(d.ino, "x", 0o644).unwrap();
        assert_eq!(st.rmdir(INodeNo::ROOT, "d").unwrap_err().code(), libc::ENOTEMPTY);

        st.unlink(d.ino, "x").unwrap();
        assert_eq!(st.getattr(INodeNo::ROOT).unwrap().nlink, 3);
        st.rmdir(INodeNo::ROOT, "d").unwrap();
        assert_eq!(st.getattr(INodeNo::ROOT).unwrap().nlink, 2);
    }

    #[test]
    fn rename_moves_file_across_dirs() {
        let mut st = fs();
        let d = st.mkdir(INodeNo::ROOT, "sub", 0o755).unwrap();
        let f = st.create(INodeNo::ROOT, "foo", 0o644).unwrap();
        st.rename(INodeNo::ROOT, "foo", d.ino, "bar").unwrap();

        assert_eq!(st.lookup(INodeNo::ROOT, "foo").unwrap_err().code(), libc::ENOENT);
        assert_eq!(st.lookup(d.ino, "bar").unwrap().ino, f.ino);
    }

    #[test]
    fn rename_dir_repoints_its_dotdot() {
        let mut st = fs();
        let a = st.mkdir(INodeNo::ROOT, "a", 0o755).unwrap();
        let b = st.mkdir(INodeNo::ROOT, "b", 0o755).unwrap();
        st.rename(INodeNo::ROOT, "a", b.ino, "a2").unwrap();

        assert_eq!(st.lookup(INodeNo::ROOT, "a").unwrap_err().code(), libc::ENOENT);
        assert_eq!(st.lookup(b.ino, "a2").unwrap().ino, a.ino);
        // `..` now resolves to new parent `b`.
        let listing = st.readdir(a.ino).unwrap();
        let dotdot = listing.iter().find(|(_, _, n)| n == "..").unwrap();
        assert_eq!(dotdot.0, b.ino);
    }

    #[test]
    fn readdir_includes_dot_dotdot_and_children() {
        let st = fs();
        let names: Vec<String> = st
            .readdir(INodeNo::ROOT)
            .unwrap()
            .into_iter()
            .map(|(_, _, n)| n)
            .collect();
        assert!(names.contains(&".".to_string()));
        assert!(names.contains(&"..".to_string()));
        assert!(names.contains(&"hello.txt".to_string()));
    }
}
