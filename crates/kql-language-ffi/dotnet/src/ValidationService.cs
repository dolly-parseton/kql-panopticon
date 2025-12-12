using Kusto.Language;
using Kusto.Language.Symbols;
using Kusto.Language.Syntax;

namespace KqlLanguageFfi;

/// <summary>
/// KQL validation service using Microsoft's Kusto.Language library.
/// </summary>
public static class ValidationService
{
    /// <summary>
    /// Validate a KQL query for syntax errors only (no schema).
    /// </summary>
    /// <param name="query">The KQL query to validate</param>
    /// <returns>Validation result with any diagnostics found</returns>
    public static ValidationResult ValidateSyntax(string query)
    {
        try
        {
            // Parse the query without schema information
            var code = KustoCode.Parse(query);

            // Get diagnostics (syntax errors only since no schema)
            var diagnostics = code.GetDiagnostics();

            return CreateResult(query, diagnostics);
        }
        catch (Exception ex)
        {
            // Return the exception as a diagnostic
            return new ValidationResult
            {
                Valid = false,
                Diagnostics = new List<Diagnostic>
                {
                    new Diagnostic
                    {
                        Message = $"Parser exception: {ex.Message}",
                        Severity = "Error",
                        Start = 0,
                        End = 0,
                        Line = 1,
                        Column = 1
                    }
                }
            };
        }
    }

    /// <summary>
    /// Validate a KQL query with schema awareness.
    /// </summary>
    /// <param name="query">The KQL query to validate</param>
    /// <param name="schema">Schema definition with tables, columns, and functions</param>
    /// <returns>Validation result with any diagnostics found</returns>
    public static ValidationResult ValidateWithSchema(string query, SchemaDefinition schema)
    {
        try
        {
            // Build GlobalState from schema
            var globals = BuildGlobalState(schema);

            // Parse and analyze with schema
            var code = KustoCode.ParseAndAnalyze(query, globals);

            // Get all diagnostics (syntax + semantic)
            var diagnostics = code.GetDiagnostics();

            return CreateResult(query, diagnostics);
        }
        catch (Exception ex)
        {
            return new ValidationResult
            {
                Valid = false,
                Diagnostics = new List<Diagnostic>
                {
                    new Diagnostic
                    {
                        Message = $"Parser exception: {ex.Message}",
                        Severity = "Error",
                        Start = 0,
                        End = 0,
                        Line = 1,
                        Column = 1
                    }
                }
            };
        }
    }

    /// <summary>
    /// Build a GlobalState from a schema definition.
    /// </summary>
    public static GlobalState BuildGlobalState(SchemaDefinition schema)
    {
        var tableSymbols = new List<TableSymbol>();

        foreach (var table in schema.Tables ?? Enumerable.Empty<TableDefinition>())
        {
            // Build column definition string: "(col1: type1, col2: type2, ...)"
            var columnDefs = string.Join(", ",
                (table.Columns ?? Enumerable.Empty<ColumnDefinition>())
                    .Select(c => $"{c.Name}: {MapDataType(c.DataType)}"));

            var tableSymbol = new TableSymbol(table.Name, $"({columnDefs})");
            tableSymbols.Add(tableSymbol);
        }

        // Build function symbols
        var functionSymbols = new List<FunctionSymbol>();
        foreach (var func in schema.Functions ?? Enumerable.Empty<FunctionDefinition>())
        {
            // Build parameter list
            var parameters = (func.Parameters ?? Enumerable.Empty<ParameterDefinition>())
                .Select(p => new Parameter(p.Name, MapScalarType(p.DataType)))
                .ToList();

            // Note: We use a simplified function definition
            // Full function bodies would require more complex handling
            var funcSymbol = new FunctionSymbol(
                func.Name,
                MapScalarType(func.ReturnType),
                parameters.ToArray());
            functionSymbols.Add(funcSymbol);
        }

        // Create database symbol with tables and functions
        var databaseName = schema.Database ?? "db";
        var members = new List<Symbol>();
        members.AddRange(tableSymbols);
        members.AddRange(functionSymbols);

        var database = new DatabaseSymbol(databaseName, members.ToArray());

        // Return globals with database
        return GlobalState.Default.WithDatabase(database);
    }

    /// <summary>
    /// Map a data type string to a Kusto type string.
    /// Handles both KQL type names and .NET type names from schema capture.
    /// </summary>
    private static string MapDataType(string? dataType)
    {
        if (string.IsNullOrEmpty(dataType))
            return "string";

        // Normalize to lowercase for matching
        return dataType.ToLowerInvariant() switch
        {
            // KQL type names
            "string" => "string",
            "long" => "long",
            "int" => "int",
            "real" => "real",
            "double" => "real",
            "bool" => "bool",
            "boolean" => "bool",
            "datetime" => "datetime",
            "date" => "datetime",
            "timespan" => "timespan",
            "guid" => "guid",
            "uuid" => "guid",
            "dynamic" => "dynamic",
            "decimal" => "decimal",

            // .NET type names (from schema capture)
            "system.string" => "string",
            "system.int64" => "long",
            "system.int32" => "int",
            "system.double" => "real",
            "system.single" => "real",
            "system.boolean" => "bool",
            "system.datetime" => "datetime",
            "system.datetimeoffset" => "datetime",
            "system.timespan" => "timespan",
            "system.guid" => "guid",
            "system.decimal" => "decimal",
            "system.object" => "dynamic",
            "system.sbyte" => "int",
            "system.byte" => "int",
            "system.int16" => "int",
            "system.uint16" => "int",
            "system.uint32" => "long",
            "system.uint64" => "long",

            _ => "dynamic" // Default to dynamic for unknown types
        };
    }

    /// <summary>
    /// Map a data type string to a ScalarSymbol.
    /// </summary>
    private static ScalarSymbol MapScalarType(string? dataType)
    {
        if (string.IsNullOrEmpty(dataType))
            return ScalarTypes.String;

        return dataType.ToLowerInvariant() switch
        {
            "string" => ScalarTypes.String,
            "long" => ScalarTypes.Long,
            "int" => ScalarTypes.Int,
            "real" => ScalarTypes.Real,
            "double" => ScalarTypes.Real,
            "bool" => ScalarTypes.Bool,
            "boolean" => ScalarTypes.Bool,
            "datetime" => ScalarTypes.DateTime,
            "date" => ScalarTypes.DateTime,
            "timespan" => ScalarTypes.TimeSpan,
            "guid" => ScalarTypes.Guid,
            "uuid" => ScalarTypes.Guid,
            "dynamic" => ScalarTypes.Dynamic,
            "decimal" => ScalarTypes.Decimal,
            _ => ScalarTypes.String // Default to string for unknown types
        };
    }

    /// <summary>
    /// Create a ValidationResult from Kusto diagnostics.
    /// </summary>
    private static ValidationResult CreateResult(string query, IReadOnlyList<Kusto.Language.Diagnostic> diagnostics)
    {
        var resultDiagnostics = new List<Diagnostic>();
        var hasErrors = false;

        foreach (var diag in diagnostics)
        {
            var (line, column) = GetLineAndColumn(query, diag.Start);
            var severity = MapSeverity(diag.Severity);

            if (severity == "Error")
                hasErrors = true;

            resultDiagnostics.Add(new Diagnostic
            {
                Message = diag.Message,
                Severity = severity,
                Start = diag.Start,
                End = diag.End,
                Line = line,
                Column = column,
                Code = diag.Code
            });
        }

        return new ValidationResult
        {
            Valid = !hasErrors,
            Diagnostics = resultDiagnostics
        };
    }

    /// <summary>
    /// Calculate line and column from a character offset.
    /// </summary>
    private static (int line, int column) GetLineAndColumn(string text, int offset)
    {
        if (offset < 0 || offset > text.Length)
            return (1, 1);

        int line = 1;
        int column = 1;

        for (int i = 0; i < offset && i < text.Length; i++)
        {
            if (text[i] == '\n')
            {
                line++;
                column = 1;
            }
            else
            {
                column++;
            }
        }

        return (line, column);
    }

    /// <summary>
    /// Map Kusto diagnostic severity to our severity string.
    /// DiagnosticSeverity in Kusto.Language is a string, not an enum.
    /// </summary>
    private static string MapSeverity(string severity)
    {
        // Kusto.Language uses string-based severity values
        return severity?.ToLowerInvariant() switch
        {
            "error" => "Error",
            "warning" => "Warning",
            "information" => "Information",
            "suggestion" => "Hint",
            _ => "Error"
        };
    }
}
