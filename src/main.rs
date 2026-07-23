//! rustFS — user-level filesystem on FUSE.
//! Run: `cargo run -- <mountpoint>`; unmount: `umount <mountpoint>`.

mod fs;
mod inode;

use std::env;

use fuser::{Config, MountOption};

use crate::fs::MemFs;

fn main() {
    let mountpoint = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: rustFS <mountpoint>");
            std::process::exit(1);
        }
    };

    let mut config = Config::default();
    config.mount_options = vec![MountOption::FSName("rustFS".to_string())];

    println!("Mounting rustFS at {mountpoint} (Ctrl-C to unmount)...");
    fuser::mount2(MemFs::new(), &mountpoint, &config).expect("failed to mount rustFS");
}
