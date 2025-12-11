#!/bin/bash
# Build script for KQL Language FFI native library
# This builds the .NET library with DNNE for native exports

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Patch runtimeconfig.json to allow major version rollforward
# This enables running on newer .NET versions (e.g., .NET 9 when targeting .NET 8)
patch_runtime_config() {
    local config_file="$1"
    if [ -f "$config_file" ]; then
        # Use sed to change rollForward from "LatestMinor" to "Major"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            # macOS sed requires empty string for in-place backup
            sed -i '' 's/"rollForward"[[:space:]]*:[[:space:]]*"[^"]*"/"rollForward": "Major"/g' "$config_file"
        else
            # GNU sed
            sed -i 's/"rollForward"[[:space:]]*:[[:space:]]*"[^"]*"/"rollForward": "Major"/g' "$config_file"
        fi
        echo "Patched runtime config: $config_file (rollForward: Major)"
    fi
}

# Default to current platform if no argument provided
if [ -z "$1" ]; then
    # Detect current platform
    case "$(uname -s)-$(uname -m)" in
        Darwin-arm64)
            RIDS=("osx-arm64")
            ;;
        Darwin-x86_64)
            RIDS=("osx-x64")
            ;;
        Linux-x86_64)
            RIDS=("linux-x64")
            ;;
        Linux-aarch64)
            RIDS=("linux-arm64")
            ;;
        MINGW*|MSYS*|CYGWIN*)
            RIDS=("win-x64")
            ;;
        *)
            echo "Unknown platform: $(uname -s)-$(uname -m)"
            exit 1
            ;;
    esac
else
    case "$1" in
        "all")
            RIDS=("osx-arm64" "osx-x64" "linux-x64" "linux-arm64" "win-x64" "win-arm64")
            ;;
        "macos")
            RIDS=("osx-arm64" "osx-x64")
            ;;
        "linux")
            RIDS=("linux-x64" "linux-arm64")
            ;;
        "windows")
            RIDS=("win-x64" "win-arm64")
            ;;
        *)
            RIDS=("$1")
            ;;
    esac
fi

echo "Building KQL Language FFI for: ${RIDS[*]}"
echo "---"

# Ensure output directories exist
mkdir -p native

for rid in "${RIDS[@]}"; do
    echo ""
    echo "Building for $rid..."
    echo "---"

    # Create output directory
    mkdir -p "native/$rid"

    # Build and publish with DNNE
    if dotnet publish -c Release -r "$rid" -o "native/$rid" 2>&1; then
        # Copy the DNNE native export library
        DNNE_DIR="obj/Release/net8.0/$rid/dnne/bin"
        case "$rid" in
            osx-*)
                if [ -f "$DNNE_DIR/KqlLanguageFfiNE.dylib" ]; then
                    cp "$DNNE_DIR/KqlLanguageFfiNE.dylib" "native/$rid/"
                    echo "Copied native library: native/$rid/KqlLanguageFfiNE.dylib"
                fi
                ;;
            linux-*)
                if [ -f "$DNNE_DIR/KqlLanguageFfiNE.so" ]; then
                    cp "$DNNE_DIR/KqlLanguageFfiNE.so" "native/$rid/"
                    echo "Copied native library: native/$rid/KqlLanguageFfiNE.so"
                fi
                ;;
            win-*)
                if [ -f "$DNNE_DIR/KqlLanguageFfiNE.dll" ]; then
                    cp "$DNNE_DIR/KqlLanguageFfiNE.dll" "native/$rid/"
                    echo "Copied native library: native/$rid/KqlLanguageFfiNE.dll"
                fi
                ;;
        esac

        # Patch runtime config for major version rollforward
        patch_runtime_config "native/$rid/KqlLanguageFfi.runtimeconfig.json"

        echo "Success: native/$rid/"
    else
        echo "Failed to build for $rid"
        exit 1
    fi
done

echo ""
echo "---"
echo "Build complete!"
echo ""
echo "Library locations:"
for rid in "${RIDS[@]}"; do
    case "$rid" in
        osx-*)
            echo "  $rid: native/$rid/KqlLanguageFfiNE.dylib"
            ;;
        linux-*)
            echo "  $rid: native/$rid/KqlLanguageFfiNE.so"
            ;;
        win-*)
            echo "  $rid: native/$rid/KqlLanguageFfiNE.dll"
            ;;
    esac
done

echo ""
echo "Required files for deployment:"
echo "  - KqlLanguageFfiNE.{dylib,so,dll} (native entry point)"
echo "  - KqlLanguageFfi.dll (managed assembly)"
echo "  - Kusto.Language.dll (Kusto parser)"
echo "  - KqlLanguageFfi.runtimeconfig.json (runtime config)"
echo "  - .NET runtime (must be installed or bundled)"
