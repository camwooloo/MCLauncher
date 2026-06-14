//! Simple world backups: zip selected subfolders of an instance/server
//! directory into a timestamped archive, list them, and restore.

use std::io::{Read, Write};
use std::path::Path;

use crate::{Error, Result};

fn io(p: &Path, e: std::io::Error) -> Error {
    Error::io(p, e)
}
fn zip_err<E: std::fmt::Display>(e: E) -> Error {
    Error::other(format!("archive error: {e}"))
}

fn add_dir<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    base: &Path,
    dir: &Path,
    opts: zip::write::SimpleFileOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let path = entry.map_err(|e| io(dir, e))?.path();
        let rel = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            let _ = zip.add_directory(format!("{rel}/"), opts);
            add_dir(zip, base, &path, opts)?;
        } else {
            zip.start_file(rel, opts).map_err(zip_err)?;
            let mut buf = Vec::new();
            std::fs::File::open(&path)
                .and_then(|mut f| f.read_to_end(&mut buf))
                .map_err(|e| io(&path, e))?;
            zip.write_all(&buf).map_err(|e| io(&path, e))?;
        }
    }
    Ok(())
}

/// Zip the given `include` subfolders of `root` into `dest`. Returns how many
/// of those folders actually existed and were added.
pub fn create(root: &Path, include: &[&str], dest: &Path) -> Result<usize> {
    if let Some(p) = dest.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let file = std::fs::File::create(dest).map_err(|e| io(dest, e))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0;
    for sub in include {
        let dir = root.join(sub);
        if dir.is_dir() {
            let _ = zip.add_directory(format!("{sub}/"), opts);
            add_dir(&mut zip, root, &dir, opts)?;
            count += 1;
        }
    }
    zip.finish().map_err(zip_err)?;
    Ok(count)
}

/// Restore a backup zip into `root`, overwriting existing files.
pub fn restore(zip_path: &Path, root: &Path) -> Result<()> {
    let file = std::fs::File::open(zip_path).map_err(|e| io(zip_path, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_err)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_err)?;
        let Some(rel) = entry.enclosed_name() else { continue }; // guards zip-slip
        let out = root.join(rel);
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out);
        } else {
            if let Some(p) = out.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            let mut f = std::fs::File::create(&out).map_err(|e| io(&out, e))?;
            std::io::copy(&mut entry, &mut f).map_err(|e| io(&out, e))?;
        }
    }
    Ok(())
}
