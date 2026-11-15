#[cfg(any(feature="nfs", feature="smb"))]
use game_network::fs as native_fs;
#[cfg(not(any(feature="nfs", feature="smb")))]
use std::fs as native_fs;

use std::path::Path;

use crate::error::{AssetError, Result};

/// Keep a local archive open between range reads. Remote and browser VFSes
/// retain their existing range-read API instead of acquiring a host file.
#[derive(Debug)]
pub(crate) struct RangeReader {
    path: std::path::PathBuf,
    #[cfg(not(any(target_arch = "wasm32", feature = "nfs", feature = "smb")))]
    file: std::sync::Mutex<std::fs::File>,
}

impl RangeReader {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            #[cfg(not(any(target_arch = "wasm32", feature = "nfs", feature = "smb")))]
            file: std::sync::Mutex::new(std::fs::File::open(path).map_err(|error| AssetError::io(path, error))?),
        })
    }

    pub(crate) fn read_range(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        #[cfg(not(any(target_arch = "wasm32", feature = "nfs", feature = "smb")))]
        {
            use std::io::{Read, Seek, SeekFrom};
            let mut file = self.file.lock().map_err(|_| AssetError::io(&self.path,
                std::io::Error::other("archive reader lock poisoned")))?;
            file.seek(SeekFrom::Start(offset as u64)).map_err(|error| AssetError::io(&self.path, error))?;
            let mut bytes = vec![0; len];
            file.read_exact(&mut bytes).map_err(|error| AssetError::io(&self.path, error))?;
            Ok(bytes)
        }
        #[cfg(any(target_arch = "wasm32", feature = "nfs", feature = "smb"))]
        read_range(&self.path, offset, len)
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub fn is_file(path: &Path) -> bool {
    native_fs::metadata(path).is_ok_and(|m|m.is_file())
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub fn file_len(path: &Path) -> Result<usize> {
    let len = native_fs::metadata(path)
        .map_err(|e| AssetError::io(path, e))?
        .len();
    usize::try_from(len).map_err(|_| AssetError::Vfs {
        path: path.to_path_buf(),
        reason: format!("file is too large for this platform: {len} bytes"),
    })
}

/// Identity used to validate a cached open. On host targets this is
/// (length, mtime); on the wasm VFS there is no mtime, so the length alone
/// is used. Callers must treat this as a best-effort freshness check, not a
/// content hash: if a file is replaced between the read and the identity
/// read, one stale (pre-replacement) entry can be served until the file is
/// touched again. This matches the existing engine contract: `PacArchive`
/// already pins its entry table at `from_file` time, so a repacked archive
/// is not picked up mid-run either.
pub fn file_identity(path: &Path) -> Result<(u64, std::time::SystemTime)> {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        let metadata = native_fs::metadata(path).map_err(|e| AssetError::io(path, e))?;
        let modified = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        Ok((metadata.len(), modified))
    }
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        let len = file_len(path)? as u64;
        Ok((len, std::time::SystemTime::UNIX_EPOCH))
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub fn read_all(path: &Path) -> Result<Vec<u8>> {
    native_fs::read(path).map_err(|e| AssetError::io(path, e))
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub fn read_range(path: &Path, offset: usize, len: usize) -> Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = native_fs::File::open(path).map_err(|e| AssetError::io(path, e))?;
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|e| AssetError::io(path, e))?;
    let mut bytes = vec![0u8; len];
    file.read_exact(&mut bytes)
        .map_err(|e| AssetError::io(path, e))?;
    Ok(bytes)
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod wasm {
    use super::*;
    use js_sys::Uint8Array;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = senaFileExists)]
        fn sena_file_exists(path: &str) -> bool;

        #[wasm_bindgen(js_name = senaFileSize)]
        fn sena_file_size(path: &str) -> f64;

        #[wasm_bindgen(js_name = senaReadFile)]
        fn sena_read_file(path: &str) -> Uint8Array;

        #[wasm_bindgen(js_name = senaReadRange)]
        fn sena_read_range(path: &str, offset: f64, len: f64) -> Uint8Array;
    }

    pub fn is_file(path: &Path) -> bool {
        sena_file_exists(&normalize_vfs_path(path))
    }

    pub fn file_len(path: &Path) -> Result<usize> {
        let normalized = normalize_vfs_path(path);
        let size = sena_file_size(&normalized);
        if !size.is_finite() || size < 0.0 {
            return Err(AssetError::Vfs {
                path: std::path::PathBuf::from(normalized),
                reason: "JavaScript VFS returned an invalid file size".to_owned(),
            });
        }
        Ok(size as usize)
    }

    pub fn read_all(path: &Path) -> Result<Vec<u8>> {
        let normalized = normalize_vfs_path(path);
        if normalized.is_empty() || !sena_file_exists(&normalized) {
            return Err(AssetError::Vfs {
                path: std::path::PathBuf::from(normalized),
                reason: "file not found".to_owned(),
            });
        }
        copy_uint8_array(sena_read_file(&normalized))
    }

    pub fn read_range(path: &Path, offset: usize, len: usize) -> Result<Vec<u8>> {
        let normalized = normalize_vfs_path(path);
        if normalized.is_empty() || !sena_file_exists(&normalized) {
            return Err(AssetError::Vfs {
                path: std::path::PathBuf::from(normalized),
                reason: "file not found".to_owned(),
            });
        }
        let bytes = copy_uint8_array(sena_read_range(&normalized, offset as f64, len as f64))?;
        if bytes.len() != len {
            return Err(AssetError::Vfs {
                path: std::path::PathBuf::from(normalized),
                reason: format!(
                    "range read returned {} bytes, expected {}",
                    bytes.len(),
                    len
                ),
            });
        }
        Ok(bytes)
    }

    fn copy_uint8_array(arr: Uint8Array) -> Result<Vec<u8>> {
        let len = arr.length() as usize;
        let mut out = vec![0u8; len];
        arr.copy_to(&mut out);
        Ok(out)
    }

    fn normalize_vfs_path(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "/")
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>()
            .join("/")
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use wasm::{file_len, is_file, read_all, read_range};

/// Local user data for network mounts; ordinary local games keep their save path.
pub fn save_directory(root:&Path)->std::path::PathBuf {
    #[cfg(any(feature="nfs", feature="smb"))]
    if let Ok(Some(user)) = game_network::fs::user_root(root) { return user; }
    root.join("save")
}
