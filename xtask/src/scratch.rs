// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scratch folders for files that must stay out of the repository, such as
//! the throwaway files that tests write (QA-09): fresh folders under the
//! system's temporary folder, each removed with everything in it once it's
//! dropped.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result};

/// A fresh folder, removed with what it holds when dropped.
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// Creates a folder that no other process or scratch folder uses.
    pub fn new() -> Result<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir();
        loop {
            let name = format!(
                "kreqo-xtask-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = base.join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                // Left behind by an earlier process with the same id.
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("couldn't create {}", path.display()));
                }
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // A drop can't report a failure, and a folder left in the system's
        // temporary folder does no harm.
        let _ = fs::remove_dir_all(&self.path);
    }
}
