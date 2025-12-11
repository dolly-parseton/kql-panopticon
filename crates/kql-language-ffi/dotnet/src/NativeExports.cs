using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace KqlLanguageFfi;

/// <summary>
/// Native FFI exports for Kusto.Language functionality.
/// Uses DNNE to export methods as native C functions.
/// </summary>
public static class NativeExports
{
    // Thread-local storage for last error message
    [ThreadStatic]
    private static string? _lastError;

    // Error codes matching Rust FFI definitions
    private const int ErrorBufferTooSmall = -1;
    private const int ErrorParseError = -2;
    private const int ErrorInternal = -3;

    /// <summary>
    /// Initialize the library. Should be called once before any other functions.
    /// </summary>
    /// <returns>0 on success, negative error code on failure</returns>
    [UnmanagedCallersOnly(EntryPoint = "kql_init")]
    public static int Init()
    {
        try
        {
            // Warm up the Kusto parser by parsing a simple query
            // This ensures all static initialization is done
            var _ = ValidationService.ValidateSyntax("T | take 1");
            return 0;
        }
        catch (Exception ex)
        {
            _lastError = $"Initialization failed: {ex}";
            return ErrorInternal;
        }
    }

    /// <summary>
    /// Cleanup the library. Should be called when done.
    /// </summary>
    [UnmanagedCallersOnly(EntryPoint = "kql_cleanup")]
    public static void Cleanup()
    {
        // Currently no cleanup needed - the .NET runtime handles memory management
        // This is here for future use and symmetry with kql_init
    }

    /// <summary>
    /// Validate KQL query syntax (without schema awareness).
    /// </summary>
    [UnmanagedCallersOnly(EntryPoint = "kql_validate_syntax")]
    public static unsafe int ValidateSyntax(
        byte* queryPtr,
        int queryLen,
        byte* outputPtr,
        int outputMaxLen)
    {
        try
        {
            // Convert input bytes to string
            var query = Encoding.UTF8.GetString(queryPtr, queryLen);

            // Validate
            var result = ValidationService.ValidateSyntax(query);

            // Serialize result to JSON
            return WriteJsonResult(result, outputPtr, outputMaxLen);
        }
        catch (Exception ex)
        {
            _lastError = $"ValidateSyntax failed: {ex}";
            return ErrorInternal;
        }
    }

    /// <summary>
    /// Validate KQL query with schema awareness.
    /// </summary>
    [UnmanagedCallersOnly(EntryPoint = "kql_validate_with_schema")]
    public static unsafe int ValidateWithSchema(
        byte* queryPtr,
        int queryLen,
        byte* schemaPtr,
        int schemaLen,
        byte* outputPtr,
        int outputMaxLen)
    {
        try
        {
            // Convert input bytes to strings
            var query = Encoding.UTF8.GetString(queryPtr, queryLen);
            var schemaJson = Encoding.UTF8.GetString(schemaPtr, schemaLen);

            // Parse schema
            var schema = JsonSerializer.Deserialize<SchemaDefinition>(schemaJson);
            if (schema == null)
            {
                _lastError = "Failed to parse schema JSON";
                return ErrorParseError;
            }

            // Validate with schema
            var result = ValidationService.ValidateWithSchema(query, schema);

            // Serialize result to JSON
            return WriteJsonResult(result, outputPtr, outputMaxLen);
        }
        catch (JsonException ex)
        {
            _lastError = $"Schema JSON parse error: {ex.Message}";
            return ErrorParseError;
        }
        catch (Exception ex)
        {
            _lastError = $"ValidateWithSchema failed: {ex}";
            return ErrorInternal;
        }
    }

    /// <summary>
    /// Get syntax classifications for a KQL query (for highlighting).
    /// </summary>
    [UnmanagedCallersOnly(EntryPoint = "kql_get_classifications")]
    public static unsafe int GetClassifications(
        byte* queryPtr,
        int queryLen,
        byte* outputPtr,
        int outputMaxLen)
    {
        try
        {
            // Convert input bytes to string
            var query = Encoding.UTF8.GetString(queryPtr, queryLen);

            // Get classifications
            var result = ClassificationService.GetClassifications(query);

            // Serialize result to JSON
            return WriteJsonResult(result, outputPtr, outputMaxLen);
        }
        catch (Exception ex)
        {
            _lastError = $"GetClassifications failed: {ex}";
            return ErrorInternal;
        }
    }

    /// <summary>
    /// Get completion items at cursor position.
    /// </summary>
    [UnmanagedCallersOnly(EntryPoint = "kql_get_completions")]
    public static unsafe int GetCompletions(
        byte* queryPtr,
        int queryLen,
        int cursorPosition,
        byte* schemaPtr,
        int schemaLen,
        byte* outputPtr,
        int outputMaxLen)
    {
        try
        {
            // Convert input bytes to string
            var query = Encoding.UTF8.GetString(queryPtr, queryLen);

            // Parse schema if provided
            SchemaDefinition? schema = null;
            if (schemaPtr != null && schemaLen > 0)
            {
                var schemaJson = Encoding.UTF8.GetString(schemaPtr, schemaLen);
                schema = JsonSerializer.Deserialize<SchemaDefinition>(schemaJson);
            }

            // Get completions
            var result = CompletionService.GetCompletions(query, cursorPosition, schema);

            // Serialize result to JSON
            return WriteJsonResult(result, outputPtr, outputMaxLen);
        }
        catch (JsonException ex)
        {
            _lastError = $"Schema JSON parse error: {ex.Message}";
            return ErrorParseError;
        }
        catch (Exception ex)
        {
            _lastError = $"GetCompletions failed: {ex}";
            return ErrorInternal;
        }
    }

    /// <summary>
    /// Get the last error message.
    /// </summary>
    [UnmanagedCallersOnly(EntryPoint = "kql_get_last_error")]
    public static unsafe int GetLastError(byte* outputPtr, int outputMaxLen)
    {
        if (string.IsNullOrEmpty(_lastError))
        {
            return 0;
        }

        var bytes = Encoding.UTF8.GetBytes(_lastError);
        if (bytes.Length > outputMaxLen)
        {
            return ErrorBufferTooSmall;
        }

        fixed (byte* src = bytes)
        {
            Buffer.MemoryCopy(src, outputPtr, outputMaxLen, bytes.Length);
        }

        // Clear the error after retrieval
        var length = bytes.Length;
        _lastError = null;
        return length;
    }

    /// <summary>
    /// Write a result object as JSON to the output buffer.
    /// </summary>
    private static unsafe int WriteJsonResult<T>(T result, byte* outputPtr, int outputMaxLen)
    {
        var json = JsonSerializer.Serialize(result, JsonOptions.Default);
        var bytes = Encoding.UTF8.GetBytes(json);

        if (bytes.Length > outputMaxLen)
        {
            _lastError = $"Output buffer too small: needed {bytes.Length}, got {outputMaxLen}";
            return ErrorBufferTooSmall;
        }

        fixed (byte* src = bytes)
        {
            Buffer.MemoryCopy(src, outputPtr, outputMaxLen, bytes.Length);
        }

        return bytes.Length;
    }
}

/// <summary>
/// JSON serialization options.
/// </summary>
internal static class JsonOptions
{
    public static readonly JsonSerializerOptions Default = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = false
    };
}
