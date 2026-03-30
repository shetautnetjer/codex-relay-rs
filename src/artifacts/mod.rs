//! Artifact handling module.

use crate::relay_core::{RelayError, RelayResult};
use crate::types::{ArtifactId, MetaMap};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

#[derive(Debug, Clone)]
pub struct ArtifactRecord {
    pub id: ArtifactId,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub bytes: Vec<u8>,
    pub metadata: MetaMap,
}

pub trait ArtifactStore: Send + Sync {
    fn put_bytes(
        &self,
        filename: String,
        mime_type: String,
        bytes: Vec<u8>,
        metadata: MetaMap,
    ) -> RelayResult<ArtifactRecord>;

    fn get(&self, id: &ArtifactId) -> RelayResult<ArtifactRecord>;
    fn delete(&self, id: &ArtifactId) -> RelayResult<()>;
}

#[derive(Debug, Clone)]
pub struct ArtifactPolicy {
    pub max_size_bytes: u64,
    pub allowed_mime_types: HashSet<String>,
}

impl ArtifactPolicy {
    pub fn validate(&self, mime_type: &str, size_bytes: u64) -> RelayResult<()> {
        if size_bytes > self.max_size_bytes {
            return Err(RelayError::Artifact(format!(
                "artifact too large: {} > {}",
                size_bytes, self.max_size_bytes
            )));
        }

        if !self.allowed_mime_types.is_empty() && !self.allowed_mime_types.contains(mime_type) {
            return Err(RelayError::Artifact(format!(
                "mime type not allowed: {mime_type}"
            )));
        }

        Ok(())
    }
}

/// Filesystem layout for job-scoped artifact operations:
/// - inbound/
/// - working/
/// - outbound/
/// - temp/
/// - logs/
#[derive(Debug, Clone)]
pub struct ArtifactFs {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct JobArtifactPaths {
    pub job_root: PathBuf,
    pub inbound: PathBuf,
    pub working: PathBuf,
    pub outbound: PathBuf,
    pub temp: PathBuf,
    pub logs: PathBuf,
}

impl ArtifactFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn ensure_job_dirs(&self, job_id: &str) -> RelayResult<JobArtifactPaths> {
        let job_root = self.root.join(job_id);
        let paths = JobArtifactPaths {
            inbound: job_root.join("inbound"),
            working: job_root.join("working"),
            outbound: job_root.join("outbound"),
            temp: job_root.join("temp"),
            logs: job_root.join("logs"),
            job_root,
        };

        fs::create_dir_all(&paths.inbound).map_err(|e| RelayError::Artifact(e.to_string()))?;
        fs::create_dir_all(&paths.working).map_err(|e| RelayError::Artifact(e.to_string()))?;
        fs::create_dir_all(&paths.outbound).map_err(|e| RelayError::Artifact(e.to_string()))?;
        fs::create_dir_all(&paths.temp).map_err(|e| RelayError::Artifact(e.to_string()))?;
        fs::create_dir_all(&paths.logs).map_err(|e| RelayError::Artifact(e.to_string()))?;
        Ok(paths)
    }

    /// Stores an uploaded zip into inbound/ and returns the copied path.
    pub fn store_inbound_zip(
        &self,
        job_id: &str,
        source_zip_path: &Path,
        filename: &str,
    ) -> RelayResult<PathBuf> {
        let paths = self.ensure_job_dirs(job_id)?;
        let target = paths.inbound.join(filename);
        fs::copy(source_zip_path, &target).map_err(|e| RelayError::Artifact(e.to_string()))?;
        Ok(target)
    }

    /// Safe extraction with zip-slip checks into working/ directory.
    pub fn extract_zip_to_working(&self, job_id: &str, zip_file: &Path) -> RelayResult<PathBuf> {
        let paths = self.ensure_job_dirs(job_id)?;
        let file = File::open(zip_file).map_err(|e| RelayError::Artifact(e.to_string()))?;
        let mut archive = ZipArchive::new(file).map_err(|e| RelayError::Artifact(e.to_string()))?;

        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| RelayError::Artifact(e.to_string()))?;
            let outpath = Self::safe_zip_target(&paths.working, entry.name())?;

            if entry.name().ends_with('/') {
                fs::create_dir_all(&outpath).map_err(|e| RelayError::Artifact(e.to_string()))?;
            } else {
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent).map_err(|e| RelayError::Artifact(e.to_string()))?;
                }
                let mut outfile =
                    File::create(&outpath).map_err(|e| RelayError::Artifact(e.to_string()))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| RelayError::Artifact(e.to_string()))?;
            }
        }

        Ok(paths.working)
    }

    /// Re-zips working/ into outbound/ and returns archive path.
    pub fn zip_working_to_outbound(&self, job_id: &str, output_name: &str) -> RelayResult<PathBuf> {
        let paths = self.ensure_job_dirs(job_id)?;
        let zip_path = paths.outbound.join(output_name);
        let file = File::create(&zip_path).map_err(|e| RelayError::Artifact(e.to_string()))?;
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        Self::add_dir_to_zip(&mut writer, &paths.working, &paths.working, options)?;
        writer
            .finish()
            .map_err(|e| RelayError::Artifact(e.to_string()))?;

        Ok(zip_path)
    }

    fn add_dir_to_zip(
        writer: &mut ZipWriter<File>,
        root: &Path,
        dir: &Path,
        options: FileOptions,
    ) -> RelayResult<()> {
        for entry in fs::read_dir(dir).map_err(|e| RelayError::Artifact(e.to_string()))? {
            let entry = entry.map_err(|e| RelayError::Artifact(e.to_string()))?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .map_err(|e| RelayError::Artifact(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");

            if path.is_dir() {
                let folder_name = format!("{name}/");
                writer
                    .add_directory(folder_name, options)
                    .map_err(|e| RelayError::Artifact(e.to_string()))?;
                Self::add_dir_to_zip(writer, root, &path, options)?;
            } else if path.is_file() {
                writer
                    .start_file(name, options)
                    .map_err(|e| RelayError::Artifact(e.to_string()))?;
                let mut f = File::open(&path).map_err(|e| RelayError::Artifact(e.to_string()))?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)
                    .map_err(|e| RelayError::Artifact(e.to_string()))?;
                writer
                    .write_all(&buf)
                    .map_err(|e| RelayError::Artifact(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Zip-slip protection helper: validates a zip entry path before extraction.
    pub fn safe_zip_target(base: &Path, entry_name: &str) -> RelayResult<PathBuf> {
        let target = base.join(entry_name);
        let normalized = normalize_path(&target);
        let normalized_base = normalize_path(base);
        if !normalized.starts_with(&normalized_base) {
            return Err(RelayError::Artifact(format!(
                "unsafe zip entry path: {entry_name}"
            )));
        }
        Ok(normalized)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            _ => out.push(comp.as_os_str()),
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryArtifactStore {
    records: Arc<Mutex<HashMap<ArtifactId, ArtifactRecord>>>,
}

impl ArtifactStore for InMemoryArtifactStore {
    fn put_bytes(
        &self,
        filename: String,
        mime_type: String,
        bytes: Vec<u8>,
        metadata: MetaMap,
    ) -> RelayResult<ArtifactRecord> {
        let id = ArtifactId(Uuid::new_v4());
        let record = ArtifactRecord {
            id: id.clone(),
            filename,
            mime_type,
            size_bytes: bytes.len() as u64,
            bytes,
            metadata,
        };

        let mut lock = self
            .records
            .lock()
            .map_err(|e| RelayError::Artifact(e.to_string()))?;
        lock.insert(id, record.clone());
        Ok(record)
    }

    fn get(&self, id: &ArtifactId) -> RelayResult<ArtifactRecord> {
        let lock = self
            .records
            .lock()
            .map_err(|e| RelayError::Artifact(e.to_string()))?;
        lock.get(id)
            .cloned()
            .ok_or_else(|| RelayError::NotFound(format!("artifact {}", id.0)))
    }

    fn delete(&self, id: &ArtifactId) -> RelayResult<()> {
        let mut lock = self
            .records
            .lock()
            .map_err(|e| RelayError::Artifact(e.to_string()))?;
        lock.remove(id);
        Ok(())
    }
}
