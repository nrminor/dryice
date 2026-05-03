//! Owned temporary-file lifecycle management.
//!
//! This example shows the recommended filesystem-backed workflow for temporary
//! `dryice` data. When `dryice` creates the temporary file, it owns the file's
//! lifecycle and removes it by default when the guard is cleaned up or dropped.
//! Use `persist` when an intermediate file turns out to be worth keeping.
//!
//! Run with: `cargo run --example temp_file_lifecycle`

use std::io::{Seek, SeekFrom};

use dryice::{DryIceReader, DryIceWriter, SeqRecord, TempDryIceFile};

fn main() -> Result<(), dryice::DryIceError> {
    let records = [
        SeqRecord::new(b"r1".to_vec(), b"ACGT".to_vec(), b"!!!!".to_vec())?,
        SeqRecord::new(b"r2".to_vec(), b"TGCA".to_vec(), b"####".to_vec())?,
    ];

    let temp = TempDryIceFile::new()?;
    println!("created temporary dryice file: {}", temp.path().display());

    let mut file = {
        let file = temp.open()?;
        let mut writer = DryIceWriter::builder().inner(file).build();
        for record in &records {
            writer.write_record(record)?;
        }
        writer.finish()?
    };

    file.seek(SeekFrom::Start(0))?;

    {
        let mut reader = DryIceReader::new(file)?;
        let mut count = 0usize;
        while reader.next_record()? {
            count += 1;
        }
        println!("read back {count} records");
    }

    // Explicit cleanup lets callers handle cleanup errors. If this line were
    // omitted, the temp-file guard would still try best-effort cleanup on drop.
    let path = temp.path().to_path_buf();
    temp.cleanup()?;
    println!("removed temporary dryice file: {}", path.display());

    Ok(())
}
