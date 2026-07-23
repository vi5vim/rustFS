# rustFS

A user-level filesystem written in Rust, built on [FUSE](https://github.com/libfuse/libfuse) via the [`fuser`](https://crates.io/crates/fuser) crate. Built to deepen understanding of filesystem internals.

> **Status:** Working FS — create, write, `mkdir`, rename, and delete all work. State lives in RAM and deletes on unmount.

## Currently implemented

- Mounts a FUSE filesystem at given mountpoint.
- Inode table + per-directory entry map (files and nested directories).
- **Read path:** `lookup`, `getattr`, `readdir` (with `.`/`..`), `read`.
- **Write path:** `create`, `write` (zero-filled gaps), `setattr` (truncate, chmod, timestamps), `mkdir`, `unlink`, `rmdir`, `rename` — with proper error codes (`EEXIST`, `ENOTEMPTY`, `EISDIR`, …).
- Unit tests, runnable without mounting (`cargo test`).
- Verified end-to-end on macOS (macFUSE).

## Architecture

```
src/
├── main.rs    # parse the mountpoint arg, start the FUSE session
├── inode.rs   # the data model AND filesystem semantics (+ unit tests)
└── fs.rs      # thin FUSE glue: locks state, calls inode.rs, forms replies
```

`inode.rs` is FUSE-free: each operation is a plain method on `FsState` returning `Result<_, Errno>`, so the logic is unit-testable in isolation. `fs.rs` translates between the kernel's protocol and those methods.

## Roadmap

- [x] Read-write in-memory FS: inode table + directory map.
- [x] `create`, `mkdir`, `unlink`, `rmdir`, `rename`, `write`, `setattr`.
- [x] Nested directories, unit tests.
- [ ] Symlinks (`symlink` / `readlink`).
- [ ] Per-open file handles (proper `open`/`release`, `O_APPEND`).
- [ ] `nlink` bookkeeping for directory renames across parents.
- [ ] Persistence — on-disk format instead of RAM.
- [ ] Permissions / ownership enforcement.
- [ ] Linux support (libfuse).
- [ ] Benchmarks.

## Installation (macOS)

Requires **[macFUSE](https://macfuse.io/) 4.x+**.

1. **Install macFUSE:**
   ```bash
   brew install --cask macfuse
   ```

2. **Approve the kernel extension:** **System Settings → Privacy & Security** → allow software from *"Benjamin Fleischer"*.
   On Apple Silicon, first enable kext loading: boot into **Recovery** → **Startup Security Utility** → **Reduced Security** → check *"Allow user management of kernel extensions."* Reboot, then approve.

3. **Build:**
   ```bash
   git clone https://github.com/vi5vim/rustFS.git
   cd rustFS
   cargo build
   ```

## Usage

```bash
# Mount:
mkdir -p /tmp/rfs
cargo run -- /tmp/rfs

# In another terminal:
ls -la /tmp/rfs                 # -> hello.txt (seeded)
cat /tmp/rfs/hello.txt          # -> Hello from rustFS!
echo "it works" > /tmp/rfs/a.txt
mkdir /tmp/rfs/sub
mv /tmp/rfs/a.txt /tmp/rfs/sub/
rm /tmp/rfs/sub/a.txt && rmdir /tmp/rfs/sub

# Unmount:
umount /tmp/rfs                 # or ctrl+c
```

## Testing

```bash
cargo test
```

## License

See [LICENSE](LICENSE).
