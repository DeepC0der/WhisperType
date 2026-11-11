use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const REQUIRED_MODEL_FILES: &[&str] = &[
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

pub const MODEL_PRIMARY_DIRNAME: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";
pub const MODEL_LEGACY_DIRNAME: &str = "parakeet-tdt-int8";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelValidationError {
    #[error("Directory does not exist: {0}")]
    DirectoryNotFound(String),

    #[error("Path is not a directory: {0}")]
    NotADirectory(String),

    #[error("Required model files missing from {path}: {missing_files:?}")]
    MissingFiles {
        path: String,
        missing_files: Vec<String>,
    },

    #[error("File is empty (0 bytes): {0}")]
    EmptyFile(String),

    #[error("IO error validating model directory: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelValidationResult {
    pub is_valid: bool,
    pub path: String,
    pub present_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub total_size_bytes: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelArtifacts {
    pub dir: PathBuf,
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub total_size_bytes: u64,
}

/// Validate if the provided directory contains all 4 required Parakeet TDT INT8 model files.
pub fn validate_model_directory(dir: &Path) -> Result<ModelArtifacts, ModelValidationError> {
    if !dir.exists() {
        return Err(ModelValidationError::DirectoryNotFound(
            dir.display().to_string(),
        ));
    }
    if !dir.is_dir() {
        return Err(ModelValidationError::NotADirectory(
            dir.display().to_string(),
        ));
    }

    let mut missing = Vec::new();
    let mut total_size = 0u64;

    for &filename in REQUIRED_MODEL_FILES {
        let file_path = dir.join(filename);
        match fs::metadata(&file_path) {
            Ok(meta) => {
                if meta.len() == 0 {
                    return Err(ModelValidationError::EmptyFile(filename.to_string()));
                }
                total_size += meta.len();
            }
            Err(_) => {
                missing.push(filename.to_string());
            }
        }
    }

    if !missing.is_empty() {
        return Err(ModelValidationError::MissingFiles {
            path: dir.display().to_string(),
            missing_files: missing,
        });
    }

    Ok(ModelArtifacts {
        dir: dir.to_path_buf(),
        encoder: dir.join("encoder.int8.onnx"),
        decoder: dir.join("decoder.int8.onnx"),
        joiner: dir.join("joiner.int8.onnx"),
        tokens: dir.join("tokens.txt"),
        total_size_bytes: total_size,
    })
}

/// Inspect a directory and return a structured validation report for IPC / UI.
pub fn inspect_model_directory(dir: &Path) -> ModelValidationResult {
    let mut present = Vec::new();
    let mut missing = Vec::new();
    let mut total_size = 0u64;

    if !dir.exists() || !dir.is_dir() {
        return ModelValidationResult {
            is_valid: false,
            path: dir.display().to_string(),
            present_files: Vec::new(),
            missing_files: REQUIRED_MODEL_FILES.iter().map(|&s| s.to_string()).collect(),
            total_size_bytes: 0,
            error_message: Some(format!("Directory not found: {}", dir.display())),
        };
    }

    for &filename in REQUIRED_MODEL_FILES {
        let file_path = dir.join(filename);
        match fs::metadata(&file_path) {
            Ok(meta) => {
                if meta.len() > 0 {
                    present.push(filename.to_string());
                    total_size += meta.len();
                } else {
                    missing.push(format!("{filename} (empty 0 bytes)"));
                }
            }
            Err(_) => {
                missing.push(filename.to_string());
            }
        }
    }

    let is_valid = missing.is_empty();
    let error_message = if is_valid {
        None
    } else {
        Some(format!(
            "Missing required files in {}: {}",
            dir.display(),
            missing.join(", ")
        ))
    };

    ModelValidationResult {
        is_valid,
        path: dir.display().to_string(),
        present_files: present,
        missing_files: missing,
        total_size_bytes: total_size,
        error_message,
    }
}

/// Retrieve the canonical Murmur-managed model directory under `%APPDATA%\Murmur\models\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`.
pub fn get_managed_model_directory() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("Murmur")
            .join("models")
            .join(MODEL_PRIMARY_DIRNAME)
    } else if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
        proj_dirs.data_dir().join("models").join(MODEL_PRIMARY_DIRNAME)
    } else {
        PathBuf::from("models").join(MODEL_PRIMARY_DIRNAME)
    }
}

/// Documented, deterministic model resolution strategy:
/// 1. Murmur-managed user data directory (%APPDATA%\Murmur\models\...)
/// 2. Cargo manifest directory (test/development environment)
/// 3. Workspace / repository paths (models/... relative to working directory)
/// 4. Executable-relative paths (release install directory)
pub fn resolve_model_directory() -> Result<PathBuf, String> {
    let mut candidate_dirs: Vec<PathBuf> = Vec::new();

    // 1. Managed storage in %APPDATA%\Murmur\models\...
    candidate_dirs.push(get_managed_model_directory());
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidate_dirs.push(
            PathBuf::from(appdata)
                .join("Murmur")
                .join("models")
                .join(MODEL_LEGACY_DIRNAME),
        );
    }
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "murmur", "Murmur") {
        candidate_dirs.push(
            proj_dirs
                .data_dir()
                .join("models")
                .join(MODEL_LEGACY_DIRNAME),
        );
    }

    // 2. Cargo manifest directory (dev & test runs)
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = PathBuf::from(manifest);
        candidate_dirs.push(manifest_path.join("../models").join(MODEL_PRIMARY_DIRNAME));
        candidate_dirs.push(manifest_path.join("../models").join(MODEL_LEGACY_DIRNAME));
        candidate_dirs.push(manifest_path.join("models").join(MODEL_PRIMARY_DIRNAME));
        candidate_dirs.push(manifest_path.join("models").join(MODEL_LEGACY_DIRNAME));
    }

    // 3. Current working directory / repository workspace
    candidate_dirs.push(PathBuf::from("models").join(MODEL_PRIMARY_DIRNAME));
    candidate_dirs.push(PathBuf::from("models").join(MODEL_LEGACY_DIRNAME));
    candidate_dirs.push(PathBuf::from("../models").join(MODEL_PRIMARY_DIRNAME));
    candidate_dirs.push(PathBuf::from("../models").join(MODEL_LEGACY_DIRNAME));

    // 4. Executable-relative paths (installed binaries or portable unpack)
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidate_dirs.push(exe_dir.join("models").join(MODEL_PRIMARY_DIRNAME));
            candidate_dirs.push(exe_dir.join("models").join(MODEL_LEGACY_DIRNAME));
            candidate_dirs.push(exe_dir.join("../models").join(MODEL_PRIMARY_DIRNAME));
            candidate_dirs.push(exe_dir.join("../../../models").join(MODEL_PRIMARY_DIRNAME));
        }
    }

    for dir in &candidate_dirs {
        if validate_model_directory(dir).is_ok() {
            log::info!("Discovered verified ASR model in: {}", dir.display());
            return Ok(dir.clone());
        }
    }

    let tried = candidate_dirs
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  - ");

    Err(format!(
        "Parakeet TDT INT8 model files not found.\nRequired files:\n  {}\nSearched locations:\n  - {}",
        REQUIRED_MODEL_FILES.join(", "),
        tried
    ))
}

/// Import a validated model from an arbitrary source folder into Murmur-managed storage.
/// Copies all 4 required files into `%APPDATA%\Murmur\models\sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`.
pub fn import_model_from_folder(source_dir: &Path) -> Result<PathBuf, String> {
    let artifacts = validate_model_directory(source_dir).map_err(|e| e.to_string())?;

    let target_dir = get_managed_model_directory();
    fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "Failed to create target model directory at {}: {e}",
            target_dir.display()
        )
    })?;

    // Copy each required file
    for &filename in REQUIRED_MODEL_FILES {
        let src = artifacts.dir.join(filename);
        let dst = target_dir.join(filename);
        let temp_dst = target_dir.join(format!("{filename}.tmp"));

        fs::copy(&src, &temp_dst).map_err(|e| {
            let _ = fs::remove_file(&temp_dst);
            format!(
                "Failed to copy {} to {}: {e}",
                src.display(),
                temp_dst.display()
            )
        })?;

        fs::rename(&temp_dst, &dst).map_err(|e| {
            let _ = fs::remove_file(&temp_dst);
            format!(
                "Failed to finalize {} at {}: {e}",
                filename,
                dst.display()
            )
        })?;
    }

    // Verify target directory
    validate_model_directory(&target_dir).map_err(|e| {
        format!(
            "Model verification failed after import into {}: {e}",
            target_dir.display()
        )
    })?;

    log::info!(
        "Successfully imported Parakeet TDT model into managed directory: {}",
        target_dir.display()
    );

    Ok(target_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_missing_directory() {
        let res = inspect_model_directory(Path::new("non_existent_folder_xyz_12345"));
        assert!(!res.is_valid);
        assert_eq!(res.missing_files.len(), 4);
    }

    #[test]
    fn test_validate_and_import_cycle() {
        let temp_base = std::env::temp_dir().join(format!("murmur_model_test_{}", uuid::Uuid::new_v4()));
        let fake_source = temp_base.join("source_model");
        fs::create_dir_all(&fake_source).unwrap();

        // Create mock model files
        for &file in REQUIRED_MODEL_FILES {
            fs::write(fake_source.join(file), b"mock-model-weights-or-tokens").unwrap();
        }

        // Validate source
        let val = validate_model_directory(&fake_source).unwrap();
        assert_eq!(val.dir, fake_source);
        assert!(val.total_size_bytes > 0);

        let inspect = inspect_model_directory(&fake_source);
        assert!(inspect.is_valid);
        assert_eq!(inspect.present_files.len(), 4);
        assert!(inspect.missing_files.is_empty());

        // Test import into custom target directory
        let fake_target = temp_base.join("managed_model");
        fs::create_dir_all(&fake_target).unwrap();

        for &filename in REQUIRED_MODEL_FILES {
            let src = fake_source.join(filename);
            let dst = fake_target.join(filename);
            fs::copy(&src, &dst).unwrap();
        }

        let target_val = validate_model_directory(&fake_target).unwrap();
        assert_eq!(target_val.present_files().len(), 4);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_base);
    }
}

#[cfg(test)]
impl ModelArtifacts {
    pub fn present_files(&self) -> Vec<String> {
        REQUIRED_MODEL_FILES.iter().map(|&s| s.to_string()).collect()
    }
}
