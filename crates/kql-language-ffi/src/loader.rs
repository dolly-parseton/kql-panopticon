//! Dynamic library loading for the native KQL validator
//!
//! This module handles finding and loading the .NET AOT native library
//! across different platforms.

use crate::error::Error;
use crate::ffi::{
    symbols, KqlCleanupFn, KqlGetClassificationsFn, KqlGetCompletionsFn, KqlGetLastErrorFn,
    KqlInitFn, KqlValidateSyntaxFn, KqlValidateWithSchemaFn,
};
use libloading::Library;
use once_cell::sync::OnceCell;
use std::path::PathBuf;

/// Environment variable for specifying library path
pub const LIB_PATH_ENV: &str = "KQL_LANGUAGE_FFI_PATH";

/// Platform-specific library name (DNNE-generated native export library)
#[cfg(target_os = "macos")]
pub const LIB_NAME: &str = "KqlLanguageFfiNE.dylib";

#[cfg(target_os = "linux")]
pub const LIB_NAME: &str = "KqlLanguageFfiNE.so";

#[cfg(target_os = "windows")]
pub const LIB_NAME: &str = "KqlLanguageFfiNE.dll";

/// Get the runtime identifier for the current platform
pub fn current_rid() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "osx-arm64";

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "osx-x64";

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-x64";

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux-arm64";

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "win-x64";

    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return "win-arm64";
}

/// Find the native library path
///
/// Search order:
/// 1. `KQL_LANGUAGE_FFI_PATH` environment variable
/// 2. Same directory as the current executable
/// 3. `native/{rid}/` relative to the crate root
/// 4. Current working directory
pub fn find_library_path() -> Option<PathBuf> {
    // 1. Check environment variable
    if let Ok(path) = std::env::var(LIB_PATH_ENV) {
        let path = PathBuf::from(path);
        // If it's a file, use it directly
        if path.is_file() {
            log::debug!("Found library via {}: {:?}", LIB_PATH_ENV, path);
            return Some(path);
        }
        // If it's a directory, look for the library file in it
        if path.is_dir() {
            let lib_path = path.join(LIB_NAME);
            if lib_path.exists() {
                log::debug!("Found library in {} directory: {:?}", LIB_PATH_ENV, lib_path);
                return Some(lib_path);
            }
        }
    }

    // 2. Same directory as executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let lib_path = exe_dir.join(LIB_NAME);
            if lib_path.exists() {
                log::debug!("Found library next to executable: {:?}", lib_path);
                return Some(lib_path);
            }
        }
    }

    // 3. Native directory relative to crate (for development)
    let native_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("dotnet")
        .join("native")
        .join(current_rid());
    let lib_path = native_dir.join(LIB_NAME);
    if lib_path.exists() {
        log::debug!("Found library in native directory: {:?}", lib_path);
        return Some(lib_path);
    }

    // 4. Current working directory
    let cwd_path = PathBuf::from(LIB_NAME);
    if cwd_path.exists() {
        log::debug!("Found library in current directory: {:?}", cwd_path);
        return Some(cwd_path);
    }

    log::debug!("Native library not found");
    None
}

/// Get the list of paths that were searched
pub fn searched_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Environment variable
    if let Ok(path) = std::env::var(LIB_PATH_ENV) {
        paths.push(PathBuf::from(&path));
        paths.push(PathBuf::from(path).join(LIB_NAME));
    }

    // Executable directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join(LIB_NAME));
        }
    }

    // Native directory
    let native_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("dotnet")
        .join("native")
        .join(current_rid());
    paths.push(native_dir.join(LIB_NAME));

    // Current directory
    paths.push(PathBuf::from(LIB_NAME));

    paths
}

/// Loaded library instance (singleton)
static LIBRARY: OnceCell<LoadedLibrary> = OnceCell::new();

/// Container for loaded library and function pointers
pub struct LoadedLibrary {
    /// The loaded library handle
    #[allow(dead_code)]
    library: Library,

    /// Initialize function
    pub init: KqlInitFn,

    /// Cleanup function (for future use)
    #[allow(dead_code)]
    pub cleanup: KqlCleanupFn,

    /// Validate syntax function
    pub validate_syntax: KqlValidateSyntaxFn,

    /// Get last error function
    pub get_last_error: KqlGetLastErrorFn,

    /// Validate with schema function (optional)
    pub validate_with_schema: Option<KqlValidateWithSchemaFn>,

    /// Get completions function (optional, Phase 2)
    pub get_completions: Option<KqlGetCompletionsFn>,

    /// Get classifications function (optional, Phase 3)
    pub get_classifications: Option<KqlGetClassificationsFn>,
}

// Safety: The library handle and function pointers are thread-safe
// because the .NET runtime is thread-safe and we only load the library once.
unsafe impl Send for LoadedLibrary {}
unsafe impl Sync for LoadedLibrary {}

impl LoadedLibrary {
    /// Load the library from the given path
    fn load_from(path: &PathBuf) -> Result<Self, Error> {
        log::info!("Loading KQL language library from {:?}", path);

        // Load the library
        let library = unsafe { Library::new(path) }
            .map_err(|e| Error::library_load_failed(path, e))?;

        // Load required symbols
        let init: KqlInitFn = unsafe {
            *library
                .get(symbols::KQL_INIT.as_bytes())
                .map_err(|_| Error::SymbolNotFound {
                    symbol: symbols::KQL_INIT.to_string(),
                })?
        };

        let cleanup: KqlCleanupFn = unsafe {
            *library
                .get(symbols::KQL_CLEANUP.as_bytes())
                .map_err(|_| Error::SymbolNotFound {
                    symbol: symbols::KQL_CLEANUP.to_string(),
                })?
        };

        let validate_syntax: KqlValidateSyntaxFn = unsafe {
            *library
                .get(symbols::KQL_VALIDATE_SYNTAX.as_bytes())
                .map_err(|_| Error::SymbolNotFound {
                    symbol: symbols::KQL_VALIDATE_SYNTAX.to_string(),
                })?
        };

        let get_last_error: KqlGetLastErrorFn = unsafe {
            *library
                .get(symbols::KQL_GET_LAST_ERROR.as_bytes())
                .map_err(|_| Error::SymbolNotFound {
                    symbol: symbols::KQL_GET_LAST_ERROR.to_string(),
                })?
        };

        // Load optional symbols (don't fail if not present)
        let validate_with_schema: Option<KqlValidateWithSchemaFn> = unsafe {
            library
                .get(symbols::KQL_VALIDATE_WITH_SCHEMA.as_bytes())
                .ok()
                .map(|s| *s)
        };

        let get_completions: Option<KqlGetCompletionsFn> = unsafe {
            library
                .get(symbols::KQL_GET_COMPLETIONS.as_bytes())
                .ok()
                .map(|s| *s)
        };

        let get_classifications: Option<KqlGetClassificationsFn> = unsafe {
            library
                .get(symbols::KQL_GET_CLASSIFICATIONS.as_bytes())
                .ok()
                .map(|s| *s)
        };

        log::debug!(
            "Loaded symbols: validate_with_schema={}, get_completions={}, get_classifications={}",
            validate_with_schema.is_some(),
            get_completions.is_some(),
            get_classifications.is_some()
        );

        Ok(Self {
            library,
            init,
            cleanup,
            validate_syntax,
            get_last_error,
            validate_with_schema,
            get_completions,
            get_classifications,
        })
    }

    /// Check if schema validation is supported
    pub fn supports_schema_validation(&self) -> bool {
        self.validate_with_schema.is_some()
    }

    /// Check if completion is supported
    pub fn supports_completion(&self) -> bool {
        self.get_completions.is_some()
    }

    /// Check if classification is supported
    pub fn supports_classification(&self) -> bool {
        self.get_classifications.is_some()
    }
}

/// Ensure DOTNET_ROOT is set for the .NET runtime
///
/// DNNE-based libraries require the .NET runtime, which needs DOTNET_ROOT
/// to be set on some systems (especially macOS with Homebrew).
fn ensure_dotnet_root() {
    // Skip if already set
    if std::env::var("DOTNET_ROOT").is_ok() {
        return;
    }

    // Try to find dotnet and derive DOTNET_ROOT
    if let Some(dotnet_root) = find_dotnet_root() {
        log::debug!("Auto-detected DOTNET_ROOT: {:?}", dotnet_root);
        std::env::set_var("DOTNET_ROOT", &dotnet_root);
    }
}

/// Try to find the .NET runtime root directory
fn find_dotnet_root() -> Option<PathBuf> {
    // Common locations to check
    let candidates = [
        // Homebrew on Apple Silicon
        "/opt/homebrew/Cellar/dotnet",
        // Homebrew on Intel Mac
        "/usr/local/Cellar/dotnet",
        // Standard Linux/macOS locations
        "/usr/share/dotnet",
        "/usr/local/share/dotnet",
        // Windows default
        "C:\\Program Files\\dotnet",
    ];

    // First, try to find via `dotnet --info` output
    if let Ok(output) = std::process::Command::new("dotnet")
        .args(["--info"])
        .output()
    {
        if output.status.success() {
            let info = String::from_utf8_lossy(&output.stdout);
            // Look for "Base Path:" line which contains the SDK path
            // e.g., "Base Path:   /opt/homebrew/Cellar/dotnet/9.0.8/libexec/sdk/9.0.109/"
            for line in info.lines() {
                if line.trim().starts_with("Base Path:") {
                    if let Some(path_str) = line.split(':').nth(1) {
                        let path = PathBuf::from(path_str.trim());
                        // Navigate up to find libexec (the actual runtime root)
                        // Path is like: .../libexec/sdk/X.Y.Z/ -> we want .../libexec
                        if let Some(libexec) = path.ancestors().find(|p| p.ends_with("libexec")) {
                            return Some(libexec.to_path_buf());
                        }
                        // Or try to find the dotnet root another way
                        if let Some(dotnet_dir) = path.ancestors().find(|p| {
                            p.join("dotnet").exists() || p.join("shared").exists()
                        }) {
                            return Some(dotnet_dir.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    // Fall back to checking known locations
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            // For Homebrew, we need to find the version directory with libexec
            if candidate.contains("Cellar") {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    // Find the latest version directory
                    let mut versions: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .collect();
                    versions.sort_by_key(|b| std::cmp::Reverse(b.path()));

                    if let Some(version_dir) = versions.first() {
                        let libexec = version_dir.path().join("libexec");
                        if libexec.exists() {
                            return Some(libexec);
                        }
                    }
                }
            } else if path.join("shared").exists() {
                return Some(path);
            }
        }
    }

    None
}

/// Load the library (or get cached instance)
pub fn load_library() -> Result<&'static LoadedLibrary, Error> {
    LIBRARY.get_or_try_init(|| {
        // Ensure DOTNET_ROOT is set for DNNE libraries
        ensure_dotnet_root();

        let path = find_library_path().ok_or_else(|| Error::LibraryNotFound {
            searched_paths: searched_paths(),
        })?;

        let lib = LoadedLibrary::load_from(&path)?;

        // Initialize the library
        let result = unsafe { (lib.init)() };
        if result != 0 {
            // Get error message
            let mut error_buf = vec![0u8; 1024];
            let error_len = unsafe { (lib.get_last_error)(error_buf.as_mut_ptr(), error_buf.len() as i32) };
            let message = if error_len > 0 {
                String::from_utf8_lossy(&error_buf[..error_len as usize]).to_string()
            } else {
                format!("Initialization returned error code: {}", result)
            };
            return Err(Error::InitializationFailed { message });
        }

        log::info!("KQL language library initialized successfully");
        Ok(lib)
    })
}

/// Check if the library is loaded
#[allow(dead_code)]
pub fn is_loaded() -> bool {
    LIBRARY.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_rid() {
        let rid = current_rid();
        assert!(!rid.is_empty());
        #[cfg(target_os = "macos")]
        assert!(rid.starts_with("osx-"));
        #[cfg(target_os = "linux")]
        assert!(rid.starts_with("linux-"));
        #[cfg(target_os = "windows")]
        assert!(rid.starts_with("win-"));
    }

    #[test]
    fn test_searched_paths_not_empty() {
        let paths = searched_paths();
        assert!(!paths.is_empty());
    }
}
