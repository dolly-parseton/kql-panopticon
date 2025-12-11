using System.Text.Json.Serialization;

namespace KqlLanguageFfi;

/// <summary>
/// Result of validating a KQL query.
/// This is serialized to JSON and returned via FFI.
/// </summary>
public class ValidationResult
{
    /// <summary>
    /// Whether the query is valid (no errors).
    /// </summary>
    [JsonPropertyName("valid")]
    public bool Valid { get; set; }

    /// <summary>
    /// List of diagnostics (errors, warnings, etc.).
    /// </summary>
    [JsonPropertyName("diagnostics")]
    public List<Diagnostic> Diagnostics { get; set; } = new();
}

/// <summary>
/// A diagnostic message from validation.
/// </summary>
public class Diagnostic
{
    /// <summary>
    /// The diagnostic message.
    /// </summary>
    [JsonPropertyName("message")]
    public string Message { get; set; } = "";

    /// <summary>
    /// Severity level: "Error", "Warning", "Information", "Hint".
    /// </summary>
    [JsonPropertyName("severity")]
    public string Severity { get; set; } = "Error";

    /// <summary>
    /// Start offset in the query (0-based character position).
    /// </summary>
    [JsonPropertyName("start")]
    public int Start { get; set; }

    /// <summary>
    /// End offset in the query (0-based character position).
    /// </summary>
    [JsonPropertyName("end")]
    public int End { get; set; }

    /// <summary>
    /// Line number (1-based).
    /// </summary>
    [JsonPropertyName("line")]
    public int Line { get; set; }

    /// <summary>
    /// Column number (1-based).
    /// </summary>
    [JsonPropertyName("column")]
    public int Column { get; set; }

    /// <summary>
    /// Error code (if available).
    /// </summary>
    [JsonPropertyName("code")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Code { get; set; }
}

/// <summary>
/// Schema definition for semantic validation.
/// Matches the Rust Schema struct.
/// </summary>
public class SchemaDefinition
{
    /// <summary>
    /// Database name (optional).
    /// </summary>
    [JsonPropertyName("database")]
    public string? Database { get; set; }

    /// <summary>
    /// Tables in the schema.
    /// </summary>
    [JsonPropertyName("tables")]
    public List<TableDefinition>? Tables { get; set; }

    /// <summary>
    /// Functions in the schema.
    /// </summary>
    [JsonPropertyName("functions")]
    public List<FunctionDefinition>? Functions { get; set; }
}

/// <summary>
/// Table definition.
/// </summary>
public class TableDefinition
{
    /// <summary>
    /// Table name.
    /// </summary>
    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    /// <summary>
    /// Columns in the table.
    /// </summary>
    [JsonPropertyName("columns")]
    public List<ColumnDefinition>? Columns { get; set; }

    /// <summary>
    /// Optional description.
    /// </summary>
    [JsonPropertyName("description")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Description { get; set; }
}

/// <summary>
/// Column definition.
/// </summary>
public class ColumnDefinition
{
    /// <summary>
    /// Column name.
    /// </summary>
    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    /// <summary>
    /// Data type (e.g., "string", "long", "datetime", "dynamic").
    /// </summary>
    [JsonPropertyName("data_type")]
    public string? DataType { get; set; }

    /// <summary>
    /// Optional description.
    /// </summary>
    [JsonPropertyName("description")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Description { get; set; }
}

/// <summary>
/// Function definition.
/// </summary>
public class FunctionDefinition
{
    /// <summary>
    /// Function name.
    /// </summary>
    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    /// <summary>
    /// Parameters.
    /// </summary>
    [JsonPropertyName("parameters")]
    public List<ParameterDefinition>? Parameters { get; set; }

    /// <summary>
    /// Return type.
    /// </summary>
    [JsonPropertyName("return_type")]
    public string ReturnType { get; set; } = "dynamic";

    /// <summary>
    /// Optional function body.
    /// </summary>
    [JsonPropertyName("body")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Body { get; set; }

    /// <summary>
    /// Optional description.
    /// </summary>
    [JsonPropertyName("description")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Description { get; set; }
}

/// <summary>
/// Parameter definition.
/// </summary>
public class ParameterDefinition
{
    /// <summary>
    /// Parameter name.
    /// </summary>
    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    /// <summary>
    /// Parameter data type.
    /// </summary>
    [JsonPropertyName("data_type")]
    public string? DataType { get; set; }

    /// <summary>
    /// Optional default value.
    /// </summary>
    [JsonPropertyName("default_value")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? DefaultValue { get; set; }
}

// ============================================================================
// Classification Types (Phase 2)
// ============================================================================

/// <summary>
/// Result of syntax classification for highlighting.
/// </summary>
public class ClassificationResult
{
    /// <summary>
    /// List of classified spans.
    /// </summary>
    [JsonPropertyName("spans")]
    public List<ClassifiedSpan> Spans { get; set; } = new();
}

/// <summary>
/// A classified span with a kind for syntax highlighting.
/// </summary>
public class ClassifiedSpan
{
    /// <summary>
    /// Start offset (0-based character position).
    /// </summary>
    [JsonPropertyName("start")]
    public int Start { get; set; }

    /// <summary>
    /// Length of the span.
    /// </summary>
    [JsonPropertyName("length")]
    public int Length { get; set; }

    /// <summary>
    /// Classification kind (matches Kusto.Language.ClassificationKind).
    /// </summary>
    [JsonPropertyName("kind")]
    public string Kind { get; set; } = "PlainText";
}

// ============================================================================
// Completion Types (Phase 2)
// ============================================================================

/// <summary>
/// Result of completion request.
/// </summary>
public class CompletionResult
{
    /// <summary>
    /// List of completion items.
    /// </summary>
    [JsonPropertyName("items")]
    public List<CompletionItemResponse> Items { get; set; } = new();
}

/// <summary>
/// A completion item for intellisense.
/// </summary>
public class CompletionItemResponse
{
    /// <summary>
    /// Display label for the completion.
    /// </summary>
    [JsonPropertyName("label")]
    public string Label { get; set; } = "";

    /// <summary>
    /// Kind of completion (Keyword, Function, Table, Column, etc.).
    /// </summary>
    [JsonPropertyName("kind")]
    public string Kind { get; set; } = "Other";

    /// <summary>
    /// Text to insert (if different from label).
    /// </summary>
    [JsonPropertyName("insert_text")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? InsertText { get; set; }

    /// <summary>
    /// Brief description or signature.
    /// </summary>
    [JsonPropertyName("detail")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Detail { get; set; }

    /// <summary>
    /// Sort order (lower = higher priority).
    /// </summary>
    [JsonPropertyName("sort_order")]
    public int SortOrder { get; set; }

    /// <summary>
    /// Character position where replacement should start.
    /// </summary>
    [JsonPropertyName("edit_start")]
    public int EditStart { get; set; }
}
