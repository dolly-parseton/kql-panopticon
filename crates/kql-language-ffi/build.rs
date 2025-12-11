//! Build script for kql-language-ffi
//!
//! This script automatically builds the .NET native library if:
//! 1. The native library doesn't exist
//! 2. The .NET SDK is available
//!
//! If the .NET SDK isn't available, it provides helpful instructions.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Set rerun triggers for .NET source files
    println!("cargo:rerun-if-changed=dotnet/src/");
    println!("cargo:rerun-if-changed=dotnet/KqlLanguageFfi.csproj");
    println!("cargo:rerun-if-changed=dotnet/build.sh");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dotnet_dir = manifest_dir.join("dotnet");

    // Determine current platform RID
    let rid = current_rid();
    let lib_name = native_lib_name();

    // Check if native library already exists
    let native_lib_path = dotnet_dir.join("native").join(rid).join(lib_name);

    if native_lib_path.exists() {
        println!(
            "cargo:warning=Native library found at {}",
            native_lib_path.display()
        );
        return;
    }

    // Check if KQL_LANGUAGE_FFI_PATH is set (user-provided library)
    if env::var("KQL_LANGUAGE_FFI_PATH").is_ok() {
        println!("cargo:warning=KQL_LANGUAGE_FFI_PATH is set, skipping native build");
        return;
    }

    // Native library doesn't exist - try to build it
    println!("cargo:warning=Native library not found, attempting to build...");

    // Check if dotnet SDK is available
    if !is_dotnet_available() {
        print_dotnet_instructions(rid, lib_name);
        return;
    }

    // Run the build script
    let build_script = dotnet_dir.join("build.sh");

    if !build_script.exists() {
        println!("cargo:warning=Build script not found at {}", build_script.display());
        println!("cargo:warning=Please run the build manually from the dotnet directory");
        return;
    }

    println!("cargo:warning=Building native library for {}...", rid);

    let output = Command::new("bash")
        .arg(&build_script)
        .arg(rid)
        .current_dir(&dotnet_dir)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            // Verify the library was actually created
            if native_lib_path.exists() {
                println!("cargo:warning=Native library built successfully");
                println!(
                    "cargo:warning=Native library available at {}",
                    native_lib_path.display()
                );
            } else {
                // Build claimed success but library doesn't exist
                println!("cargo:warning=Build script completed but library not found!");
                println!("cargo:warning=Expected: {}", native_lib_path.display());

                // Print build output for debugging
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                if !stdout.is_empty() {
                    for line in stdout.lines().take(20) {
                        println!("cargo:warning=[dotnet] {}", line);
                    }
                }
                if !stderr.is_empty() {
                    for line in stderr.lines().take(10) {
                        println!("cargo:warning=[dotnet-err] {}", line);
                    }
                }
                print_manual_build_instructions(rid);
            }
        }
        Ok(result) => {
            println!(
                "cargo:warning=Native library build failed with exit code: {:?}",
                result.status.code()
            );

            // Print error output
            let stderr = String::from_utf8_lossy(&result.stderr);
            for line in stderr.lines().take(10) {
                println!("cargo:warning=[dotnet-err] {}", line);
            }
            print_manual_build_instructions(rid);
        }
        Err(e) => {
            println!("cargo:warning=Failed to run build script: {}", e);
            print_manual_build_instructions(rid);
        }
    }
}

/// Get the runtime identifier for the current platform
fn current_rid() -> &'static str {
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

/// Get the native library filename for the current platform
fn native_lib_name() -> &'static str {
    #[cfg(target_os = "macos")]
    return "KqlLanguageFfiNE.dylib";

    #[cfg(target_os = "linux")]
    return "KqlLanguageFfiNE.so";

    #[cfg(target_os = "windows")]
    return "KqlLanguageFfiNE.dll";
}

/// Check if the dotnet SDK is available
fn is_dotnet_available() -> bool {
    Command::new("dotnet")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Print instructions for installing .NET SDK
fn print_dotnet_instructions(rid: &str, lib_name: &str) {
    println!("cargo:warning=");
    println!("cargo:warning======================================================");
    println!("cargo:warning=.NET SDK not found - cannot build native library");
    println!("cargo:warning======================================================");
    println!("cargo:warning=");
    println!("cargo:warning=The kql-language-ffi crate requires a native library built from .NET.");
    println!("cargo:warning=");
    println!("cargo:warning=Options:");
    println!("cargo:warning=");
    println!("cargo:warning=1. Install .NET 8.0+ SDK and rebuild:");
    println!("cargo:warning=   - macOS: brew install dotnet");
    println!("cargo:warning=   - Linux: https://docs.microsoft.com/dotnet/core/install/linux");
    println!("cargo:warning=   - Windows: https://dotnet.microsoft.com/download");
    println!("cargo:warning=");
    println!("cargo:warning=2. Set KQL_LANGUAGE_FFI_PATH to a pre-built library:");
    println!("cargo:warning=   export KQL_LANGUAGE_FFI_PATH=/path/to/{}", lib_name);
    println!("cargo:warning=");
    println!("cargo:warning=3. Download pre-built binaries from releases (if available)");
    println!("cargo:warning=");
    println!("cargo:warning=Target platform: {} ({})", rid, lib_name);
    println!("cargo:warning======================================================");
}

/// Print instructions for manual build
fn print_manual_build_instructions(rid: &str) {
    println!("cargo:warning=");
    println!("cargo:warning=To build manually, run:");
    println!("cargo:warning=  cd crates/kql-language-ffi/dotnet && ./build.sh {}", rid);
}
