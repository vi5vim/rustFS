# rustFS

A user-level filesystem written in Rust, built on [FUSE](https://github.com/libfuse/libfuse)
via the [`fuser`](https://crates.io/crates/fuser) crate. Built to deepen understanding
of filesystem internals — inodes, directory entries, the VFS/kernel handoff — while
producing something you can actually mount and use.

> **Status:** A working **read-write, in-memory** filesystem — create, write,
> `mkdir`, rename, and delete all work. State lives in RAM and is gone on
> unmount (persistence is the next milestone). See roadmap below.

## Currently implemented

- Mounts a real FUSE filesystem at a given mountpoint (`cargo run -- <mountpoint>`).
- An inode table + per-directory entry map (files and nested directories).
- **Read path:** `lookup`, `getattr`, `readdir` (with `.`/`..`), `read`.
- **Write path:** `create`, `write` (with zero-filled gaps), `setattr` (truncate,
  chmod, timestamps), `mkdir`, `unlink`, `rmdir`, `rename` (including across
  directories), with proper error codes (`EEXIST`, `ENOTEMPTY`, `EISDIR`, …).
- Unit tests covering the filesystem semantics, runnable without mounting
  (`cargo test`).
- Verified end-to-end on macOS (macFUSE).

## Architecture

```
src/
├── main.rs    # parse the mountpoint arg, start the FUSE session
├── inode.rs   # the data model AND filesystem semantics (+ unit tests)
└── fs.rs      # thin FUSE glue: locks state, calls inode.rs, forms replies
```

`inode.rs` is deliberately FUSE-free: each operation is a plain method on
`FsState` returning `Result<_, Errno>`, so the logic is unit-testable in
isolation. `fs.rs` only translates between the kernel's protocol and those
methods.

## Roadmap

- [x] **Read-write in-memory FS** — a real inode table + directory map.
- [x] File/dir creation & removal: `create`, `mkdir`, `unlink`, `rmdir`, `rename`.
- [x] `write` and `setattr` (including truncate).
- [x] Multiple files and nested directories.
- [x] Unit tests for the data model.
- [ ] Symlinks (`symlink` / `readlink`).
- [ ] Per-open file handles (proper `open`/`release`, `O_APPEND`).
- [ ] `nlink` bookkeeping for directory renames across parents.
- [ ] Persistence — back the FS with an on-disk format rather than RAM.
- [ ] Permissions / ownership enforcement.
- [ ] Linux support (libfuse) alongside macOS.
- [ ] Benchmarks.

## Requirements

- **macOS**
- **[macFUSE](https://macfuse.io/) 4.x+**

## Installation (macOS)

1. **Install macFUSE** via Homebrew:
   ```bash
   brew install --cask macfuse
   ```
   (Or download the installer at https://macfuse.io/)

2. **Approve macFUSE kernel extension**
   - Open **System Settings → Privacy & Security**, find blocked software from
     *"Benjamin Fleischer"*, and click **Allow**.
   - On Apple Silicon you may first need to enable kext loading: reboot into
     **Recovery** (hold the power button on startup) → **Startup Security Utility** →
     select your disk → **Reduced Security** → check *"Allow user management of kernel
     extensions from identified developers."* Reboot, then approve as above.

3. **Build:**
   ```bash
   git clone https://github.com/vi5vim/rustFS.git
   cd rustFS
   cargo build
   ```

## Usage

```bash
# Create a mountpoint and mount the filesystem:
mkdir -p /tmp/rfs
cargo run -- /tmp/rfs

# In another terminal — it's a real read-write filesystem:
ls -la /tmp/rfs                 # -> hello.txt (seeded)
cat /tmp/rfs/hello.txt          # -> Hello from rustFS!
echo "it works" > /tmp/rfs/a.txt
cat /tmp/rfs/a.txt              # -> it works
mkdir /tmp/rfs/sub
mv /tmp/rfs/a.txt /tmp/rfs/sub/
rm /tmp/rfs/sub/a.txt && rmdir /tmp/rfs/sub

# Unmount when done:
umount /tmp/rfs                 # or ctrl+c
```

## Testing

The filesystem semantics are unit-tested without needing to mount:

```bash
cargo test
```

## License

See [LICENSE](LICENSE).
