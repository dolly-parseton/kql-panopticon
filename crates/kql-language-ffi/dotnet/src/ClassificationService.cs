using Kusto.Language;
using Kusto.Language.Symbols;
using Kusto.Language.Syntax;

namespace KqlLanguageFfi;

/// <summary>
/// KQL syntax classification service for syntax highlighting.
/// Uses Microsoft's Kusto.Language library with semantic analysis to classify tokens.
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
            // Parse AND analyze the query for semantic information
            // This gives us access to ReferencedSymbol which tells us exactly what each identifier is
            var code = KustoCode.ParseAndAnalyze(query);
            var spans = new List<ClassifiedSpan>();

            // Walk the syntax tree and classify each token using semantic info
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
    /// Classify an identifier or keyword based on semantic analysis.
    /// Uses ReferencedSymbol from Kusto.Language's semantic analysis.
    /// </summary>
    private static string ClassifyIdentifierOrKeyword(SyntaxToken token, SyntaxElement? parent)
    {
        if (parent == null)
            return "Identifier";

        // Walk up the tree to find the first element with a ReferencedSymbol
        // Do this BEFORE keyword checks so function calls like count() get classified correctly
        var current = parent;
        while (current != null)
        {
            // Check for FunctionCallExpression (for function names like 'ago', 'count', etc.)
            if (current is FunctionCallExpression funcCall && funcCall.ReferencedSymbol != null)
            {
                // Only return this for the function name itself, not for arguments
                var nameExpr = funcCall.Name;
                if (nameExpr != null && IsDescendantOrSelf(nameExpr, token))
                {
                    return ClassifySymbol(funcCall.ReferencedSymbol);
                }
            }

            // Check for NameReference
            if (current is NameReference nameRef && nameRef.ReferencedSymbol != null)
            {
                return ClassifySymbol(nameRef.ReferencedSymbol);
            }

            // Check for generic Expression with ReferencedSymbol
            if (current is Expression expr && expr.ReferencedSymbol != null)
            {
                return ClassifySymbol(expr.ReferencedSymbol);
            }

            current = current.Parent;
        }

        // Query operators (where, project, summarize, etc.)
        // Check after semantic analysis so things like count() are classified as functions
        if (IsQueryOperatorKeyword(token))
            return "QueryOperator";

        // Keywords
        if (IsKeyword(token.Kind))
            return "Keyword";

        return "Identifier";
    }

    /// <summary>
    /// Check if a token is contained within or is the same as the given element.
    /// </summary>
    private static bool IsDescendantOrSelf(SyntaxElement ancestor, SyntaxToken descendant)
    {
        var current = descendant as SyntaxElement;
        while (current != null)
        {
            if (current == ancestor)
                return true;
            current = current.Parent;
        }
        return false;
    }

    /// <summary>
    /// Classify based on the resolved symbol type from semantic analysis.
    /// </summary>
    private static string ClassifySymbol(Symbol symbol)
    {
        return symbol switch
        {
            TableSymbol => "Table",
            ColumnSymbol => "Column",
            FunctionSymbol fs => fs.IsAggregate() ? "AggregateFunction" : "ScalarFunction",
            VariableSymbol => "Variable",
            ParameterSymbol => "Parameter",
            DatabaseSymbol => "Database",
            ClusterSymbol => "Cluster",
            ScalarSymbol => "Literal",  // Built-in scalar types
            _ => "Identifier"
        };
    }

    /// <summary>
    /// Check if a FunctionSymbol is an aggregate using GlobalState.
    /// </summary>
    private static bool IsAggregate(this FunctionSymbol fs)
    {
        // Use GlobalState's IsAggregateFunction which checks against the known aggregates collection
        return GlobalState.Default.IsAggregateFunction(fs);
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
            "find" or "search" or "getschema" or "as" or "by" or "on" or
            "let" or "set" or "alias" or "declare" or "pattern" or
            "restrict" or "access" => true,
            _ => false
        };
    }
}
