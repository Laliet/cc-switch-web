use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use tempfile::{tempdir, TempDir};
use zip::write::SimpleFileOptions;
use zip::DateTime;

use crate::error::AppError;
use crate::services::skill::SkillService;

use super::{MAX_SYNC_ARTIFACT_BYTES, REMOTE_SKILLS_ZIP};

const MAX_EXTRACT_ENTRIES: usize = 10_000;

pub(super) struct SkillsBackup {
    _tmp: TempDir,
    backup_dir: PathBuf,
    ssot_path: PathBuf,
    existed: bool,
}

pub(super) fn zip_skills_ssot(dest_path: &Path) -> Result<(), AppError> {
    let source = SkillService::get_ssot_dir()
        .map_err(|error| AppError::Config(format!("Failed to resolve Skills SSOT: {error}")))?;
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
    }

    let file = fs::File::create(dest_path).map_err(|error| AppError::io(dest_path, error))?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());

    if source.exists() {
        let root = fs::canonicalize(&source).unwrap_or(source);
        let mut visited = HashSet::new();
        mark_visited_dir(&root, &mut visited)?;
        zip_dir_recursive(&root, &root, &mut writer, options, &mut visited)?;
    }
    writer
        .finish()
        .map_err(|error| AppError::Config(format!("Failed to write skills.zip: {error}")))?;
    Ok(())
}

pub(super) fn backup_current_skills() -> Result<SkillsBackup, AppError> {
    let ssot_path = SkillService::get_ssot_dir()
        .map_err(|error| AppError::Config(format!("Failed to resolve Skills SSOT: {error}")))?;
    let tmp = tempdir().map_err(|source| AppError::IoContext {
        context: "Failed to create temporary Skills backup directory".to_string(),
        source,
    })?;
    let backup_dir = tmp.path().join("skills-backup");
    let existed = ssot_path.exists();
    if existed {
        copy_dir_recursive(&ssot_path, &backup_dir)?;
    }
    Ok(SkillsBackup {
        _tmp: tmp,
        backup_dir,
        ssot_path,
        existed,
    })
}

pub(super) fn restore_skills_from_backup(backup: &SkillsBackup) -> Result<(), AppError> {
    if backup.ssot_path.exists() {
        fs::remove_dir_all(&backup.ssot_path)
            .map_err(|error| AppError::io(&backup.ssot_path, error))?;
    }
    if backup.existed {
        copy_dir_recursive(&backup.backup_dir, &backup.ssot_path)?;
    }
    Ok(())
}

pub(super) fn restore_skills_zip(raw: &[u8]) -> Result<(), AppError> {
    let tmp = tempdir().map_err(|source| AppError::IoContext {
        context: "Failed to create temporary Skills extraction directory".to_string(),
        source,
    })?;
    let zip_path = tmp.path().join(REMOTE_SKILLS_ZIP);
    fs::write(&zip_path, raw).map_err(|error| AppError::io(&zip_path, error))?;
    let file = fs::File::open(&zip_path).map_err(|error| AppError::io(&zip_path, error))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| AppError::Config(format!("Invalid skills.zip: {error}")))?;
    if archive.len() > MAX_EXTRACT_ENTRIES {
        return Err(AppError::InvalidInput(format!(
            "skills.zip contains too many entries (maximum {MAX_EXTRACT_ENTRIES})"
        )));
    }

    let extracted = tmp.path().join("skills-extracted");
    fs::create_dir_all(&extracted).map_err(|error| AppError::io(&extracted, error))?;
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::Config(format!("Invalid ZIP entry: {error}")))?;
        let Some(name) = entry.enclosed_name() else {
            return Err(AppError::InvalidInput(
                "skills.zip contains an unsafe path".to_string(),
            ));
        };
        let output = extracted.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|error| AppError::io(&output, error))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| AppError::io(parent, error))?;
        }
        let mut file = fs::File::create(&output).map_err(|error| AppError::io(&output, error))?;
        copy_entry_with_limit(&mut entry, &mut file, &mut total_bytes, &output)?;
    }

    let ssot = SkillService::get_ssot_dir()
        .map_err(|error| AppError::Config(format!("Failed to resolve Skills SSOT: {error}")))?;
    let rollback = ssot.with_extension("webdav-rollback");
    if rollback.exists() {
        fs::remove_dir_all(&rollback).map_err(|error| AppError::io(&rollback, error))?;
    }
    if ssot.exists() {
        fs::rename(&ssot, &rollback).map_err(|error| AppError::io(&ssot, error))?;
    }
    if let Err(error) = copy_dir_recursive(&extracted, &ssot) {
        let _ = fs::remove_dir_all(&ssot);
        if rollback.exists() {
            let _ = fs::rename(&rollback, &ssot);
        }
        return Err(error);
    }
    let _ = fs::remove_dir_all(rollback);
    Ok(())
}

fn zip_dir_recursive(
    root: &Path,
    current: &Path,
    writer: &mut zip::ZipWriter<fs::File>,
    options: SimpleFileOptions,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), AppError> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| AppError::io(current, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::io(current, error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let real_path = fs::canonicalize(&path).map_err(|error| AppError::io(&path, error))?;
        if !real_path.starts_with(root) {
            return Err(AppError::InvalidInput(format!(
                "Skill path escapes the SSOT directory: {}",
                path.display()
            )));
        }
        let relative = real_path
            .strip_prefix(root)
            .map_err(|error| AppError::Config(format!("Invalid Skill path: {error}")))?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if real_path.is_dir() {
            if !mark_visited_dir(&real_path, visited)? {
                continue;
            }
            writer
                .add_directory(format!("{name}/"), options)
                .map_err(|error| {
                    AppError::Config(format!("Failed to add ZIP directory: {error}"))
                })?;
            zip_dir_recursive(root, &real_path, writer, options, visited)?;
        } else {
            writer
                .start_file(name, options)
                .map_err(|error| AppError::Config(format!("Failed to add ZIP file: {error}")))?;
            let mut file =
                fs::File::open(&real_path).map_err(|error| AppError::io(&real_path, error))?;
            std::io::copy(&mut file, writer).map_err(|source| AppError::IoContext {
                context: format!("Failed to archive {}", real_path.display()),
                source,
            })?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), AppError> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(destination).map_err(|error| AppError::io(destination, error))?;
    for entry in fs::read_dir(source).map_err(|error| AppError::io(source, error))? {
        let entry = entry.map_err(|error| AppError::io(source, error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| AppError::io(&destination_path, error))?;
        }
    }
    Ok(())
}

fn mark_visited_dir(path: &Path, visited: &mut HashSet<PathBuf>) -> Result<bool, AppError> {
    let canonical = fs::canonicalize(path).map_err(|error| AppError::io(path, error))?;
    Ok(visited.insert(canonical))
}

fn copy_entry_with_limit<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    total_bytes: &mut u64,
    output: &Path,
) -> Result<(), AppError> {
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| AppError::io(output, error))?;
        if read == 0 {
            return Ok(());
        }
        if total_bytes.saturating_add(read as u64) > MAX_SYNC_ARTIFACT_BYTES {
            return Err(AppError::InvalidInput(
                "skills.zip extracted size exceeds 512 MiB".to_string(),
            ));
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|error| AppError::io(output, error))?;
        *total_bytes += read as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn extraction_limit_is_enforced_before_writing_oversized_chunk() {
        let mut reader = Cursor::new(vec![1u8; 16]);
        let mut output = Vec::new();
        let mut total = MAX_SYNC_ARTIFACT_BYTES - 8;
        let error =
            copy_entry_with_limit(&mut reader, &mut output, &mut total, Path::new("skill.bin"))
                .expect_err("oversized archive must fail");
        assert!(error.to_string().contains("512 MiB"));
        assert!(output.is_empty());
    }
}
