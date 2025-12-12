using Kusto.Language;
using Kusto.Language.Editor;
using Kusto.Language.Symbols;

namespace KqlLanguageFfi;

/// <summary>
/// KQL completion service for intellisense.
/// Uses Microsoft's Kusto.Language library to provide completion suggestions.
/// </summary>
public static class CompletionService
{
    /// <summary>
    /// Get completion items at the specified cursor position.
    /// </summary>
    /// <param name="query">The KQL query</param>
    /// <param name="cursorPosition">Cursor position (0-based character offset)</param>
    /// <param name="schema">Optional schema for context-aware completions</param>
    /// <returns>Completion result with items</returns>
    public static CompletionResult GetCompletions(string query, int cursorPosition, SchemaDefinition? schema = null)
    {
        try
        {
            // Build globals with schema if provided
            GlobalState globals = schema != null
                ? ValidationService.BuildGlobalState(schema)
                : GlobalState.Default;

            // Create CodeScript from query string with globals
            var script = CodeScript.From(query, globals);

            // Get the CodeBlock at the cursor position
            var block = script.GetBlockAtPosition(cursorPosition);

            // Get completions from the block's service
            var completionInfo = block.Service.GetCompletionItems(cursorPosition);

            var items = new List<CompletionItemResponse>();
            int sortOrder = 0;

            foreach (var item in completionInfo.Items)
            {
                int editStart = completionInfo.EditStart;

                // Use MatchText for insertion if available (e.g., "ago" for label "ago(timespan)")
                // Otherwise fall back to DisplayText
                // Note: AfterText is for incremental completion and not suitable for full replacement
                string? insertText = null;
                if (!string.IsNullOrEmpty(item.MatchText) && item.MatchText != item.DisplayText)
                {
                    insertText = item.MatchText;
                }

                items.Add(new CompletionItemResponse
                {
                    Label = item.DisplayText,
                    Kind = MapCompletionKind(item.Kind),
                    InsertText = insertText,
                    Detail = GetCompletionDetail(item),
                    SortOrder = sortOrder++,
                    EditStart = editStart
                });
            }

            return new CompletionResult { Items = items };
        }
        catch (Exception)
        {
            // On error, return empty result
            return new CompletionResult();
        }
    }

    /// <summary>
    /// Map Kusto completion kind to our string representation.
    /// </summary>
    private static string MapCompletionKind(CompletionKind kind)
    {
        return kind switch
        {
            CompletionKind.Keyword => "Keyword",
            CompletionKind.Punctuation => "Punctuation",
            CompletionKind.Syntax => "Keyword",
            CompletionKind.Example => "Other",
            CompletionKind.Table => "Table",
            CompletionKind.Column => "Column",
            CompletionKind.Variable => "Variable",
            CompletionKind.Parameter => "Parameter",
            CompletionKind.Database => "Database",
            CompletionKind.Cluster => "Cluster",
            CompletionKind.AggregateFunction => "AggregateFunction",
            CompletionKind.BuiltInFunction => "Function",
            CompletionKind.LocalFunction => "Function",
            CompletionKind.DatabaseFunction => "Function",
            CompletionKind.Unknown => "Other",
            _ => "Other"
        };
    }

    /// <summary>
    /// Get detail text for a completion item (e.g., function signature).
    /// </summary>
    private static string? GetCompletionDetail(CompletionItem item)
    {
        // For items with a match text different from display text
        if (!string.IsNullOrEmpty(item.MatchText) && item.MatchText != item.DisplayText)
        {
            return item.MatchText;
        }

        return null;
    }
}
