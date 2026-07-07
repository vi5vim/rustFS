# rustFS

A user-level filesystem written in Rust, built on [FUSE](https://github.com/libfuse/libfuse)
via the [`fuser`](https://crates.io/crates/fuser) crate. Built to deepen understanding
of filesystem internals — inodes, directory entries, the VFS/kernel handoff — while
producing something you can actually mount and use.

> **Status:** Currently a minimal **read-only** filesystem that
> mounts and serves a single file. See roadmap below.

## Currently implemented

- Mounts a real FUSE filesystem at a given mountpoint (`cargo run -- <mountpoint>`).
- A read-only root directory containing one file, `hello.txt`.
- The core read-path FUSE operations:
  - `lookup` — resolve a name in a directory to an inode.
  - `getattr` — report inode attributes (the FS-level `stat`).
  - `readdir` — list directory contents (including `.` and `..`).
  - `read` — return file contents, honoring the read offset.
- Verified end-to-end on macOS (macFUSE): `ls` and `cat` against the mount work.

## Roadmap

Planned:

- [ ] **Read-write in-memory FS** — a real inode table + directory map instead of
      hardcoded entries.
- [ ] File/dir creation & removal: `create`, `mkdir`, `unlink`, `rmdir`, `rename`.
- [ ] `write` and `setattr` (including truncate).
- [ ] Multiple files and nested directories.
- [ ] Symlinks (`symlink` / `readlink`).
- [ ] Persistence — back the FS with an on-disk format rather than RAM.
- [ ] Permissions / ownership enforcement.
- [ ] Linux support (libfuse) alongside macOS.
- [ ] Tests and benchmarks.

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

# In another terminal:
ls -la /tmp/rfs          # -> hello.txt
cat /tmp/rfs/hello.txt   # -> Hello from rustFS!

# Unmount when done:
umount /tmp/rfs          # or ctrl+c
```

## License

See [LICENSE](LICENSE).
