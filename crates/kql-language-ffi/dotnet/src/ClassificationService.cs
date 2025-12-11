using Kusto.Language;
using Kusto.Language.Syntax;

namespace KqlLanguageFfi;

/// <summary>
/// KQL syntax classification service for syntax highlighting.
/// Uses Microsoft's Kusto.Language library to classify tokens.
/// </summary>
public static class ClassificationService
{
    /// <summary>
    /// Get syntax classifications for a KQL query.
    /// </summary>
    /// <param name="query">The KQL query to classify</param>
    /// <returns>Classification result with spans for each token</returns>
    public static ClassificationResult GetClassifications(string query)
    {
        try
        {
            // Parse the query
            var code = KustoCode.Parse(query);
            var spans = new List<ClassifiedSpan>();

            // Walk the syntax tree and classify each token
            ClassifyNode(code.Syntax, spans);

            return new ClassificationResult { Spans = spans };
        }
        catch (Exception)
        {
            // On error, return empty result (let validation catch errors)
            return new ClassificationResult();
        }
    }

    /// <summary>
    /// Recursively classify nodes in the syntax tree.
    /// </summary>
    private static void ClassifyNode(SyntaxNode node, List<ClassifiedSpan> spans)
    {
        // Process tokens at this node
        for (int i = 0; i < node.ChildCount; i++)
        {
            var child = node.GetChild(i);
            if (child is SyntaxToken token)
            {
                ClassifyToken(token, spans);
            }
            else if (child is SyntaxNode childNode)
            {
                ClassifyNode(childNode, spans);
            }
        }
    }

    /// <summary>
    /// Classify a single token.
    /// </summary>
    private static void ClassifyToken(SyntaxToken token, List<ClassifiedSpan> spans)
    {
        // Skip empty tokens
        if (token.Width == 0)
            return;

        // Get classification from the token's parent context
        var kind = GetClassificationKind(token);

        // Skip plain text to reduce output size (only include meaningful spans)
        if (kind == "PlainText" && token.Kind == SyntaxKind.EndOfTextToken)
            return;

        spans.Add(new ClassifiedSpan
        {
            Start = token.TextStart,
            Length = token.Width,
            Kind = kind
        });
    }

    /// <summary>
    /// Determine the classification kind for a token based on its syntax kind
    /// and parent context.
    /// </summary>
    private static string GetClassificationKind(SyntaxToken token)
    {
        var kind = token.Kind;
        var parent = token.Parent;

        // Check token kind first
        switch (kind)
        {
            // Literals
            case SyntaxKind.StringLiteralToken:
            case SyntaxKind.RawGuidLiteralToken:
                return "StringLiteral";

            case SyntaxKind.LongLiteralToken:
            case SyntaxKind.RealLiteralToken:
            case SyntaxKind.DecimalLiteralToken:
            case SyntaxKind.IntLiteralToken:
            case SyntaxKind.DateTimeLiteralToken:
            case SyntaxKind.TimespanLiteralToken:
            case SyntaxKind.GuidLiteralToken:
            case SyntaxKind.BooleanLiteralToken:
                return "Literal";

            // Punctuation
            case SyntaxKind.OpenParenToken:
            case SyntaxKind.CloseParenToken:
            case SyntaxKind.OpenBracketToken:
            case SyntaxKind.CloseBracketToken:
            case SyntaxKind.OpenBraceToken:
            case SyntaxKind.CloseBraceToken:
            case SyntaxKind.CommaToken:
            case SyntaxKind.SemicolonToken:
            case SyntaxKind.ColonToken:
            case SyntaxKind.DotToken:
            case SyntaxKind.DotDotToken:
            case SyntaxKind.FatArrowToken:
                return "Punctuation";

            // Bar/Pipe operator
            case SyntaxKind.BarToken:
                return "QueryOperator";

            // Operators
            case SyntaxKind.EqualToken:
            case SyntaxKind.EqualEqualToken:
            case SyntaxKind.BangEqualToken:
            case SyntaxKind.LessThanToken:
            case SyntaxKind.LessThanOrEqualToken:
            case SyntaxKind.GreaterThanToken:
            case SyntaxKind.GreaterThanOrEqualToken:
            case SyntaxKind.PlusToken:
            case SyntaxKind.MinusToken:
            case SyntaxKind.AsteriskToken:
            case SyntaxKind.SlashToken:
            case SyntaxKind.PercentToken:
            case SyntaxKind.EqualTildeToken:
            case SyntaxKind.BangTildeToken:
                return "ScalarOperator";

            // Comments would go here if we had them tokenized separately
        }

        // Check parent context for identifiers and keywords
        if (kind == SyntaxKind.IdentifierToken || IsKeyword(kind))
        {
            return ClassifyIdentifierOrKeyword(token, parent);
        }

        return "PlainText";
    }

    /// <summary>
    /// Classify an identifier or keyword based on parent context.
    /// </summary>
    private static string ClassifyIdentifierOrKeyword(SyntaxToken token, SyntaxElement? parent)
    {
        if (parent == null)
            return "Identifier";

        // Get the kind from the token's classification context
        var parentKind = parent.Kind;

        // Query operators (where, project, summarize, etc.)
        if (IsQueryOperatorKeyword(token))
            return "QueryOperator";

        // Check parent type for context
        switch (parent)
        {
            case NameReference nameRef:
                // Could be table, column, or function depending on resolution
                // Without semantic analysis, we use heuristics
                if (IsLikelyTableName(nameRef))
                    return "Table";
                if (IsLikelyFunctionCall(nameRef))
                    return "ScalarFunction";
                return "Column";

            case FunctionCallExpression funcCall:
                // Check if this token is the function name
                var funcName = funcCall.Name;
                if (funcName is NameReference fnRef && fnRef.SimpleName == token.Text)
                    return IsAggregateFunction(token.Text) ? "AggregateFunction" : "ScalarFunction";
                break;
        }

        // Keywords
        if (IsKeyword(token.Kind))
            return "Keyword";

        return "Identifier";
    }

    /// <summary>
    /// Check if a token kind is a keyword.
    /// </summary>
    private static bool IsKeyword(SyntaxKind kind)
    {
        // This is a simplified check - Kusto has many keywords
        return kind.ToString().EndsWith("Keyword");
    }

    /// <summary>
    /// Check if a token is a query operator keyword.
    /// </summary>
    private static bool IsQueryOperatorKeyword(SyntaxToken token)
    {
        var text = token.Text.ToLowerInvariant();
        return text switch
        {
            "where" or "project" or "extend" or "summarize" or "join" or
            "order" or "sort" or "take" or "limit" or "top" or "count" or
            "distinct" or "union" or "render" or "parse" or "mv-expand" or
            "mv-apply" or "make-series" or "lookup" or "evaluate" or
            "facet" or "sample" or "sample-distinct" or "reduce" or
            "serialize" or "invoke" or "fork" or "partition" or
            "find" or "search" or "getschema" => true,
            _ => false
        };
    }

    /// <summary>
    /// Check if an aggregate function name.
    /// </summary>
    private static bool IsAggregateFunction(string name)
    {
        var lower = name.ToLowerInvariant();
        return lower switch
        {
            "count" or "countif" or "dcount" or "dcountif" or "sum" or "sumif" or
            "avg" or "avgif" or "min" or "minif" or "max" or "maxif" or
            "stdev" or "stdevif" or "stdevp" or "variance" or "variancep" or
            "make_list" or "make_set" or "make_bag" or "make_list_if" or
            "make_set_if" or "make_bag_if" or "arg_max" or "arg_min" or
            "any" or "anyif" or "take_any" or "take_anyif" or
            "percentile" or "percentiles" or "percentile_array" or
            "hll" or "hll_merge" or "tdigest" or "tdigest_merge" => true,
            _ => false
        };
    }

    /// <summary>
    /// Heuristic: check if a name reference looks like a table name.
    /// Tables are typically at the start of a query or after certain keywords.
    /// </summary>
    private static bool IsLikelyTableName(NameReference nameRef)
    {
        var parent = nameRef.Parent;

        // At the start of a pipe expression
        if (parent is PipeExpression)
            return true;

        // After 'from' or in table expressions
        // This is a simplification - real resolution would require semantic analysis
        return false;
    }

    /// <summary>
    /// Heuristic: check if a name reference is followed by parentheses (function call).
    /// </summary>
    private static bool IsLikelyFunctionCall(NameReference nameRef)
    {
        var parent = nameRef.Parent;
        return parent is FunctionCallExpression;
    }
}
