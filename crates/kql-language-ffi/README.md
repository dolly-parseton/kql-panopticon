# kql-language-ffi

Rust FFI bindings to Microsoft's [Kusto.Language](https://www.nuget.org/packages/Microsoft.Azure.Kusto.Language/) library for KQL (Kusto Query Language) validation, completion, and syntax classification.

## Overview

This crate provides native Rust access to the official KQL parser and language services via .NET AOT compilation and FFI. It enables:

- **Syntax Validation** - Check KQL queries for syntax errors
- **Schema Validation** - Validate queries against a database schema (tables, columns, functions)
- **Completions** - Get intellisense suggestions at cursor position
- **Classification** - Get syntax highlighting spans for queries

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Rust Application                         │
│                                                              │
│  KqlValidator::new()?                                        │
│    .validate_syntax("T | where x > 1")?                     │
│    .get_completions("T | ", 4, Some(&schema))?              │
│    .get_classifications("T | take 10")?                     │
└──────────────────────────┬──────────────────────────────────┘
                           │ FFI (C ABI)
┌──────────────────────────▼──────────────────────────────────┐
│                  .NET Native Library                         │
│                  (KqlLanguageFfiNE.dylib)                    │
│                                                              │
│  Microsoft.Azure.Kusto.Language                              │
│  - KustoCode.ParseAndAnalyze()                               │
│  - CodeScript.GetCompletionItems()                           │
│  - Syntax tree classification                                │
└─────────────────────────────────────────────────────────────┘
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
kql-language-ffi = { path = "../kql-language-ffi" }
```

**Note:** The native library is built automatically by `cargo build` if not present (requires .NET SDK).

## API Reference

### KqlValidator

The main entry point for all language services.

```rust
use kql_language_ffi::{KqlValidator, Schema, Table};

// Create a validator instance (loads native library)
let validator = KqlValidator::new()?;
```

### Syntax Validation

Check a query for syntax errors without schema awareness:

```rust
let result = validator.validate_syntax("SecurityEvent | where TimeGenerated > ago(1h)")?;

if result.is_valid() {
    println!("Query is valid!");
} else {
    for diagnostic in result.diagnostics() {
        println!("{}:{}: {} - {}",
            diagnostic.line,
            diagnostic.column,
            diagnostic.severity,
            diagnostic.message
        );
    }
}
```

### Schema Validation

Validate queries against a known schema:

```rust
let schema = Schema::new()
    .table(Table::new("SecurityEvent")
        .with_column("TimeGenerated", "datetime")
        .with_column("Account", "string")
        .with_column("Computer", "string"));

let result = validator.validate_with_schema(
    "SecurityEvent | project TimeGenerated, UnknownColumn",
    &schema
)?;

// Will report error: "UnknownColumn" not found
assert!(!result.is_valid());
```

### Completions (Intellisense)

Get completion suggestions at a cursor position:

```rust
// Without schema - returns keywords and operators
let completions = validator.get_completions("SecurityEvent | ", 16, None)?;

for item in &completions.items {
    println!("{} ({:?})", item.label, item.kind);
    // "where (Keyword)", "project (Keyword)", "summarize (Keyword)", etc.
}

// With schema - includes table and column names
let completions = validator.get_completions(
    "SecurityEvent | project ",
    24,
    Some(&schema)
)?;
// Returns: TimeGenerated, Account, Computer, plus functions...
```

**CompletionItem fields:**
- `label` - Display text
- `kind` - `Keyword`, `Function`, `Table`, `Column`, etc.
- `insert_text` - Text to insert (if different from label)
- `detail` - Brief description or signature
- `sort_order` - Priority (lower = higher priority)
- `edit_start` - Character position where replacement starts

### Classification (Syntax Highlighting)

Get classified spans for syntax highlighting:

```rust
let result = validator.get_classifications("SecurityEvent | where x > 1")?;

for span in &result.spans {
    println!("{}..{}: {:?}",
        span.start,
        span.start + span.length,
        span.kind
    );
}
// 0..13: Identifier (SecurityEvent)
// 14..15: QueryOperator (|)
// 16..21: QueryOperator (where)
// 22..23: Identifier (x)
// 24..25: ScalarOperator (>)
// 26..27: Literal (1)
```

**ClassificationKind variants:**
- `PlainText`, `Comment`, `Punctuation`, `Directive`
- `Literal`, `StringLiteral`, `Type`, `Identifier`
- `Column`, `Table`, `Database`, `Cluster`
- `ScalarFunction`, `AggregateFunction`
- `Keyword`, `Operator`, `Variable`, `Parameter`
- `QueryOperator`, `ScalarOperator`

## Types

### ValidationResult

```rust
pub struct ValidationResult {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    fn is_valid(&self) -> bool;
    fn has_errors(&self) -> bool;
    fn has_warnings(&self) -> bool;
    fn errors(&self) -> impl Iterator<Item = &Diagnostic>;
    fn warnings(&self) -> impl Iterator<Item = &Diagnostic>;
}
```

### Diagnostic

```rust
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,  // Error, Warning, Information, Hint
    pub start: usize,   // Character offset
    pub end: usize,
    pub line: usize,    // 1-based
    pub column: usize,  // 1-based
    pub code: Option<String>,
}
```

### Schema

```rust
let schema = Schema::new()
    .with_database("MyDatabase")
    .table(Table::new("Events")
        .with_column("Timestamp", "datetime")
        .with_column("Message", "string")
        .with_column("Level", "int")
        .with_description("Application events"))
    .function(Function::new("GetRecentEvents")
        .with_parameter("hours", "int")
        .with_return_type("dynamic")
        .with_body("Events | where Timestamp > ago(hours * 1h)"));
```

## Building the Native Library

### Automatic Build (Recommended)

The native library is built automatically when you run `cargo build`:

```bash
cargo build -p kql-language-ffi
```

The build script will:
1. Check if the native library already exists
2. If not, detect if .NET SDK is available
3. Automatically run the build script to compile the native library
4. Provide helpful instructions if .NET SDK is not installed

### Prerequisites

- [.NET 8.0 SDK](https://dotnet.microsoft.com/download/dotnet/8.0) or later
- Platform-specific C compiler (Xcode on macOS, gcc on Linux, MSVC on Windows)

### Manual Build (Optional)

If you prefer to build manually or need cross-platform builds:

```bash
cd crates/kql-language-ffi/dotnet

# Build for current platform
./build.sh

# Build for specific platform
./build.sh osx-arm64
./build.sh linux-x64

# Build all platforms (requires cross-compilation setup)
./build.sh all
```

**Supported platforms:**
- `osx-arm64` - macOS Apple Silicon
- `osx-x64` - macOS Intel
- `linux-x64` - Linux x86_64
- `linux-arm64` - Linux ARM64
- `win-x64` - Windows x86_64
- `win-arm64` - Windows ARM64

### Output

The build produces:
```
dotnet/native/{rid}/
├── KqlLanguageFfiNE.dylib    # Native entry point (or .so/.dll)
├── KqlLanguageFfi.dll         # Managed assembly
├── Kusto.Language.dll         # Kusto parser
└── KqlLanguageFfi.runtimeconfig.json
```

## Testing

### Run Tests

The library auto-detects `DOTNET_ROOT` on most systems (including Homebrew installations):

```bash
# Run all tests (including integration tests that require native library)
cargo test -p kql-language-ffi -- --include-ignored

# Run with output visible
cargo test -p kql-language-ffi -- --include-ignored --nocapture

# Run specific test
cargo test -p kql-language-ffi test_get_completions -- --include-ignored
```

### Manual DOTNET_ROOT (if auto-detection fails)

If tests fail with runtime errors, manually set `DOTNET_ROOT`:

```bash
# macOS with Homebrew
export DOTNET_ROOT=/opt/homebrew/Cellar/dotnet/9.0.8/libexec

# Or find your .NET installation
export DOTNET_ROOT=$(dirname $(dirname $(which dotnet)))
```

### Test Coverage

| Test | Description |
|------|-------------|
| `test_validate_syntax_valid` | Valid query returns no errors |
| `test_validate_syntax_invalid` | Invalid query returns errors |
| `test_validate_with_schema` | Schema-aware validation passes |
| `test_validate_with_schema_unknown_column` | Unknown column detected |
| `test_get_classifications` | Syntax spans returned |
| `test_get_completions_after_pipe` | Operators suggested after `\|` |
| `test_get_completions_with_schema` | Columns suggested with schema |

## Library Loading

The native library is searched in this order:

1. `KQL_LANGUAGE_FFI_PATH` environment variable (file or directory)
2. Same directory as the executable
3. `dotnet/native/{rid}/` relative to the crate
4. Current working directory

To override:
```bash
export KQL_LANGUAGE_FFI_PATH=/path/to/native/osx-arm64
```

## FFI Contract

For consumers building their own bindings, the C ABI functions are:

```c
// Lifecycle
int32_t kql_init(void);
void kql_cleanup(void);

// Validation
int32_t kql_validate_syntax(
    const uint8_t* query, int32_t query_len,
    uint8_t* output, int32_t output_max_len
);

int32_t kql_validate_with_schema(
    const uint8_t* query, int32_t query_len,
    const uint8_t* schema_json, int32_t schema_len,
    uint8_t* output, int32_t output_max_len
);

// Completions
int32_t kql_get_completions(
    const uint8_t* query, int32_t query_len,
    int32_t cursor_position,
    const uint8_t* schema_json, int32_t schema_len,  // nullable
    uint8_t* output, int32_t output_max_len
);

// Classification
int32_t kql_get_classifications(
    const uint8_t* query, int32_t query_len,
    uint8_t* output, int32_t output_max_len
);

// Error retrieval
int32_t kql_get_last_error(uint8_t* output, int32_t output_max_len);
```

**Return codes:**
- `> 0` - Success, value is JSON length written to output
- `0` - Success, empty/valid result
- `-1` - Buffer too small
- `-2` - Parse error in input
- `-3` - Internal error

## Platform Support

| Platform | Build | Test | Status |
|----------|-------|------|--------|
| macOS ARM64 | Yes | Yes | Verified |
| macOS x64 | Yes | - | Untested |
| Linux x64 | Yes | - | Untested |
| Linux ARM64 | Yes | - | Untested |
| Windows x64 | Yes | - | Untested |
| Windows ARM64 | Yes | - | Untested |

## License

This crate is part of the kql-panopticon project.

The Kusto.Language library is provided by Microsoft under its own license terms.
