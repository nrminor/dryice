//! Owned temporary files for filesystem-backed `dryice` workflows.
//!
//! The core reader and writer APIs stay generic over [`std::io::Read`] and
//! [`std::io::Write`]. This module adds a small ownership layer for workflows
//! where `dryice` itself should create the backing file and clean it up when it
//! is no longer needed.
//!
//! Cleanup is best-effort on drop: failures are logged with `log::warn!` and
//! never panic. Call [`TempDryIceFile::cleanup`] when cleanup errors need to be
//! handled explicitly.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::DryIceError;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// An owned temporary `dryice` file that evaporates by default.
///
/// `TempDryIceFile` owns a filesystem path created by `dryice`. The file is
/// removed when [`cleanup`](Self::cleanup) is called, or on drop as a
/// best-effort fallback. Use [`persist`](Self::persist) to move the file into a
/// caller-owned location and disable automatic cleanup.
pub struct TempDryIceFile {
    path: PathBuf,
    cleanup_on_drop: bool,
}

impl TempDryIceFile {
    /// Create a temporary `dryice` file in the system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a temporary file cannot be created.
    pub fn new() -> Result<Self, DryIceError> {
        Self::new_in(std::env::temp_dir())
    }

    /// Create a temporary `dryice` file in `directory`.
    ///
    /// The directory must already exist. The file is created with exclusive
    /// creation semantics to avoid reusing an existing path.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory does not exist, if permissions prevent
    /// file creation, or if a unique temporary path cannot be created after
    /// repeated attempts.
    pub fn new_in<P: AsRef<Path>>(directory: P) -> Result<Self, DryIceError> {
        let directory = directory.as_ref();
        let path = create_unique_temp_file(directory)?;
        Ok(Self {
            path,
            cleanup_on_drop: true,
        })
    }

    /// Return the owned temporary file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Open the owned temporary file for reading and writing.
    ///
    /// The returned [`File`] is a normal Rust file handle. It can be passed into
    /// [`DryIceWriter`](crate::DryIceWriter), returned by `writer.finish()`,
    /// rewound, and then passed into [`DryIceReader`](crate::DryIceReader):
    ///
    /// ```
    /// use std::io::{Seek, SeekFrom};
    ///
    /// use dryice::{DryIceWriter, SeqRecord, TempDryIceFile};
    ///
    /// # fn example() -> Result<(), dryice::DryIceError> {
    /// let temp = TempDryIceFile::new()?;
    /// let file = temp.open()?;
    /// let mut writer = DryIceWriter::builder().inner(file).build();
    /// let record = SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec())?;
    /// writer.write_record(&record)?;
    /// let mut file = writer.finish()?;
    /// file.seek(SeekFrom::Start(0))?;
    /// temp.cleanup()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the temporary file cannot be opened.
    pub fn open(&self) -> Result<File, DryIceError> {
        Ok(OpenOptions::new().read(true).write(true).open(&self.path)?)
    }

    /// Remove the temporary file now.
    ///
    /// Missing files are treated as already cleaned up. If the file has already
    /// been persisted, cleanup is a no-op because `dryice` no longer owns the
    /// file lifecycle.
    ///
    /// # Errors
    ///
    /// Returns an error if removing the temporary file fails for reasons other
    /// than the file already being absent.
    pub fn cleanup(mut self) -> Result<(), DryIceError> {
        if !self.cleanup_on_drop {
            return Ok(());
        }

        remove_temp_file(&self.path)?;
        self.cleanup_on_drop = false;
        Ok(())
    }

    /// Move the temporary file into a caller-owned path.
    ///
    /// After a successful persist, `dryice` no longer owns the file lifecycle
    /// and will not remove the destination on drop. The destination must not
    /// already exist.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` already exists or if the temporary file cannot
    /// be renamed to `path`.
    pub fn persist<P: AsRef<Path>>(&mut self, path: P) -> Result<PathBuf, DryIceError> {
        let destination = path.as_ref().to_path_buf();
        if destination.exists() {
            return Err(DryIceError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "persist destination already exists",
            )));
        }

        fs::rename(&self.path, &destination)?;
        self.cleanup_on_drop = false;
        self.path.clone_from(&destination);
        Ok(destination)
    }
}

impl Drop for TempDryIceFile {
    fn drop(&mut self) {
        if !self.cleanup_on_drop {
            return;
        }

        if let Err(error) = remove_temp_file(&self.path) {
            log::warn!(
                "failed to clean up temporary dryice file `{}`: {error}",
                self.path.display()
            );
        }
    }
}

fn create_unique_temp_file(directory: &Path) -> Result<PathBuf, DryIceError> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for _ in 0..100 {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = directory.join(format!("dryice-{pid}-{nanos}-{counter}.dryice"));

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                drop(file);
                return Ok(candidate);
            },
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {},
            Err(error) => return Err(DryIceError::Io(error)),
        }
    }

    Err(DryIceError::Io(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique temporary dryice file",
    )))
}

fn remove_temp_file(path: &Path) -> Result<(), DryIceError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DryIceError::Io(error)),
    }
}
