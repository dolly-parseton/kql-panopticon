//! Syntax highlighting for the editor
//!
//! Provides a `Highlighter` trait and implementations for KQL and YAML.
//!
//! ## Color Scheme
//!
//! KQL highlighting uses the following colors:
//! - QueryOperator (where, project, summarize): Cyan
//! - Keyword (by, on, and, or): Magenta
//! - Table (SecurityEvent, SigninLogs): Yellow
//! - Column (TimeGenerated, Account): Green
//! - ScalarFunction (ago, now): Blue
//! - AggregateFunction (count, sum): Blue + Bold
//! - StringLiteral ("values"): Red
//! - Literal (numbers, true/false): Cyan
//! - Comment (// ...): DarkGray

use ratatui::style::{Color, Modifier, Style};

/// A highlighted span in the text
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    /// Start offset (0-based, in bytes)
    pub start: usize,
    /// Length in bytes
    pub length: usize,
    /// Style to apply
    pub style: Style,
}

/// Trait for syntax highlighting implementations
pub trait Highlighter: Send + Sync {
    /// Highlight the given content and return spans
    fn highlight(&self, content: &str) -> Vec<HighlightSpan>;
}

/// No-op highlighter (for testing or when highlighting unavailable)
pub struct NoOpHighlighter;

impl Highlighter for NoOpHighlighter {
    fn highlight(&self, _content: &str) -> Vec<HighlightSpan> {
        Vec::new()
    }
}

/// KQL syntax highlighter using the FFI bindings
pub struct KqlHighlighter {
    /// The validator instance for getting classifications
    validator: Option<kql_panopticon_core::validation::KqlValidator>,
}

impl KqlHighlighter {
    /// Create a new KQL highlighter
    pub fn new() -> Self {
        let validator = kql_panopticon_core::validation::KqlValidator::new().ok();
        if validator.is_none() {
            log::warn!("KQL validator not available - syntax highlighting disabled");
        }
        Self { validator }
    }

    /// Map a classification kind to a style
    fn classification_to_style(kind: kql_panopticon_core::validation::ClassificationKind) -> Style {
        use kql_panopticon_core::validation::ClassificationKind;

        match kind {
            // Query operators: cyan (where, project, summarize, extend, etc.)
            ClassificationKind::QueryOperator => Style::default().fg(Color::Cyan),

            // Keywords: magenta (by, on, and, or, not, etc.)
            ClassificationKind::Keyword => Style::default().fg(Color::Magenta),

            // Tables: yellow
            ClassificationKind::Table => Style::default().fg(Color::Yellow),

            // Columns: green
            ClassificationKind::Column => Style::default().fg(Color::Green),

            // Scalar functions: blue (ago, now, datetime_diff, etc.)
            ClassificationKind::ScalarFunction => Style::default().fg(Color::Blue),

            // Aggregate functions: blue + bold (count, sum, avg, etc.)
            ClassificationKind::AggregateFunction => {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            }

            // String literals: red
            ClassificationKind::StringLiteral => Style::default().fg(Color::Red),

            // Other literals (numbers, booleans): cyan
            ClassificationKind::Literal => Style::default().fg(Color::Cyan),

            // Comments: dark gray
            ClassificationKind::Comment => Style::default().fg(Color::DarkGray),

            // Operators (==, >, <, +, -): white (default)
            ClassificationKind::Operator | ClassificationKind::ScalarOperator => {
                Style::default().fg(Color::White)
            }

            // Punctuation: default
            ClassificationKind::Punctuation => Style::default(),

            // Identifiers: default
            ClassificationKind::Identifier => Style::default(),

            // Variables and parameters: cyan italic
            ClassificationKind::Variable | ClassificationKind::Parameter => {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC)
            }

            // Database/cluster: yellow dim
            ClassificationKind::Database | ClassificationKind::Cluster => {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM)
            }

            // Command keywords: magenta bold
            ClassificationKind::CommandKeyword => {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            }

            // Types: blue
            ClassificationKind::Type => Style::default().fg(Color::Blue),

            // Plugins: green
            ClassificationKind::Plugin => Style::default().fg(Color::Green),

            // Materialized views: yellow
            ClassificationKind::MaterializedViewFunction => Style::default().fg(Color::Yellow),

            // Options and directives: magenta dim
            ClassificationKind::Option | ClassificationKind::Directive | ClassificationKind::ClientDirective => {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::DIM)
            }

            // Query parameters: cyan
            ClassificationKind::QueryParameter => Style::default().fg(Color::Cyan),

            // Plain text: no style
            ClassificationKind::PlainText => Style::default(),
        }
    }
}

impl Default for KqlHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter for KqlHighlighter {
    fn highlight(&self, content: &str) -> Vec<HighlightSpan> {
        let Some(validator) = &self.validator else {
            log::trace!("KqlHighlighter: No validator available");
            return Vec::new();
        };

        // Check if classification is supported
        if !validator.supports_classification() {
            log::trace!("KqlHighlighter: Classification not supported");
            return Vec::new();
        }

        // Get classifications from the FFI
        match validator.get_classifications(content) {
            Ok(result) => {
                log::trace!("KqlHighlighter: Got {} spans for content len {}", result.spans.len(), content.len());
                result
                    .spans
                    .into_iter()
                    .map(|span| HighlightSpan {
                        start: span.start,
                        length: span.length,
                        style: Self::classification_to_style(span.kind),
                    })
                    .collect()
            }
            Err(e) => {
                log::debug!("Classification failed: {}", e);
                Vec::new()
            }
        }
    }
}

/// YAML syntax highlighter (simple keyword-based)
pub struct YamlHighlighter;

impl YamlHighlighter {
    /// Create a new YAML highlighter
    pub fn new() -> Self {
        Self
    }
}

impl Default for YamlHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter for YamlHighlighter {
    fn highlight(&self, content: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        let mut offset = 0;

        for line in content.lines() {
            // Find key: value patterns
            if let Some(colon_pos) = line.find(':') {
                let key_start = line.chars().take_while(|c| c.is_whitespace()).count();

                // Key styling (before colon): cyan
                if colon_pos > key_start {
                    spans.push(HighlightSpan {
                        start: offset + key_start,
                        length: colon_pos - key_start,
                        style: Style::default().fg(Color::Cyan),
                    });
                }

                // Value styling (after colon)
                let value_start = colon_pos + 1;
                let value = line[value_start..].trim();

                if !value.is_empty() {
                    let value_offset = offset + line.find(value).unwrap_or(value_start);
                    let style = yaml_value_style(value);
                    spans.push(HighlightSpan {
                        start: value_offset,
                        length: value.len(),
                        style,
                    });
                }
            }

            // Highlight comments
            if let Some(comment_start) = line.find('#') {
                // Make sure it's not inside a string
                let before_hash = &line[..comment_start];
                let quote_count = before_hash.matches('"').count() + before_hash.matches('\'').count();
                if quote_count % 2 == 0 {
                    spans.push(HighlightSpan {
                        start: offset + comment_start,
                        length: line.len() - comment_start,
                        style: Style::default().fg(Color::DarkGray),
                    });
                }
            }

            offset += line.len() + 1; // +1 for newline
        }

        spans
    }
}

/// Get the style for a YAML value
fn yaml_value_style(value: &str) -> Style {
    // Check for string literals
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return Style::default().fg(Color::Red);
    }

    // Check for booleans
    if value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("no")
    {
        return Style::default().fg(Color::Magenta);
    }

    // Check for null
    if value.eq_ignore_ascii_case("null") || value == "~" {
        return Style::default().fg(Color::Magenta).add_modifier(Modifier::DIM);
    }

    // Check for numbers
    if value.parse::<f64>().is_ok() {
        return Style::default().fg(Color::Cyan);
    }

    // Default: green for unquoted strings
    Style::default().fg(Color::Green)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_highlighter_key_value() {
        let highlighter = YamlHighlighter::new();
        let content = "name: test";
        let spans = highlighter.highlight(content);

        // Should have spans for key and value
        assert!(!spans.is_empty());

        // Key should be cyan
        let key_span = spans.iter().find(|s| s.start == 0).unwrap();
        assert_eq!(key_span.style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_yaml_highlighter_comment() {
        let highlighter = YamlHighlighter::new();
        let content = "name: test  # this is a comment";
        let spans = highlighter.highlight(content);

        // Should have a comment span
        let comment_span = spans.iter().find(|s| s.style.fg == Some(Color::DarkGray));
        assert!(comment_span.is_some());
    }

    #[test]
    fn test_yaml_value_styles() {
        assert_eq!(yaml_value_style("\"quoted\"").fg, Some(Color::Red));
        assert_eq!(yaml_value_style("true").fg, Some(Color::Magenta));
        assert_eq!(yaml_value_style("false").fg, Some(Color::Magenta));
        assert_eq!(yaml_value_style("null").fg, Some(Color::Magenta));
        assert_eq!(yaml_value_style("123").fg, Some(Color::Cyan));
        assert_eq!(yaml_value_style("3.14").fg, Some(Color::Cyan));
        assert_eq!(yaml_value_style("unquoted").fg, Some(Color::Green));
    }

    #[test]
    fn test_noop_highlighter() {
        let highlighter = NoOpHighlighter;
        let spans = highlighter.highlight("any content");
        assert!(spans.is_empty());
    }

    #[test]
    #[ignore = "requires native library"]
    fn test_kql_highlighter() {
        let highlighter = KqlHighlighter::new();

        // Check if validator was created
        assert!(highlighter.validator.is_some(), "Validator should be created");

        let validator = highlighter.validator.as_ref().unwrap();
        println!("supports_classification: {}", validator.supports_classification());

        let spans = highlighter.highlight("SecurityEvent | where TimeGenerated > ago(1h)");

        println!("Got {} spans", spans.len());
        for span in &spans {
            println!("  span at {}..{}", span.start, span.start + span.length);
        }

        assert!(!spans.is_empty(), "KQL highlighter should return spans");
    }

    #[test]
    #[ignore = "requires native library"]
    fn test_classification_kinds() {
        use kql_panopticon_core::validation::KqlValidator;

        let validator = KqlValidator::new().expect("Validator should be created");

        let query = "SecurityEvent | where TimeGenerated > ago(1h) | summarize count() by Account";
        println!("\nQuery: {}", query);
        println!("\nClassifications:");

        let result = validator
            .get_classifications(query)
            .expect("Classification should succeed");

        for span in &result.spans {
            let text = &query[span.start..span.start + span.length];
            println!("  '{}' ({}..{}) = {:?}", text, span.start, span.start + span.length, span.kind);
        }

        // Verify key classifications
        let find_kind = |text: &str| -> Option<kql_panopticon_core::validation::ClassificationKind> {
            result.spans.iter().find_map(|s| {
                let span_text = &query[s.start..s.start + s.length];
                if span_text == text {
                    Some(s.kind)
                } else {
                    None
                }
            })
        };

        use kql_panopticon_core::validation::ClassificationKind;

        // Built-in functions should be properly classified
        assert_eq!(find_kind("ago"), Some(ClassificationKind::ScalarFunction), "ago should be ScalarFunction");
        assert_eq!(find_kind("count"), Some(ClassificationKind::AggregateFunction), "count should be AggregateFunction");

        // Columns are resolved from context (without explicit schema, these are inferred from usage)
        assert_eq!(find_kind("TimeGenerated"), Some(ClassificationKind::Column), "TimeGenerated should be Column");
        assert_eq!(find_kind("Account"), Some(ClassificationKind::Column), "Account should be Column");

        // Tables without schema context will be classified as Identifier
        // (SecurityEvent is not a built-in table - would need schema from Azure Sentinel)
        // This is expected behavior - we'd need to pass schema to get Table classification
        assert_eq!(find_kind("SecurityEvent"), Some(ClassificationKind::Identifier), "SecurityEvent without schema is Identifier");
    }

    #[test]
    #[ignore = "requires native library"]
    fn test_kql_highlighter_partial_query() {
        let highlighter = KqlHighlighter::new();

        // Test with partial/incomplete KQL (like what user types during editing)
        let test_cases = vec![
            "Usage",
            "Usage\n| where ",
            "T",
            "SecurityEvent |",
            "SecurityEvent | where ",
        ];

        for content in test_cases {
            let spans = highlighter.highlight(content);
            println!("Content '{}': {} spans", content.replace('\n', "\\n"), spans.len());
            for span in &spans {
                println!("  span at {}..{} (len {})", span.start, span.start + span.length, span.length);
            }
        }
    }
}
