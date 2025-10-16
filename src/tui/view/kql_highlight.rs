use ratatui::{
    style::{Color, Style},
    text::Span,
};

/// KQL keyword categories
const KQL_KEYWORDS: &[&str] = &[
    "let",
    "print",
    "where",
    "project",
    "extend",
    "summarize",
    "join",
    "union",
    "sort",
    "top",
    "limit",
    "take",
    "distinct",
    "sample",
    "count",
    "as",
    "by",
    "on",
    "kind",
    "inner",
    "leftouter",
    "rightouter",
    "fullouter",
    "leftanti",
    "rightanti",
    "leftsemi",
    "rightsemi",
    "asc",
    "desc",
    "nulls",
    "first",
    "last",
    "render",
    "evaluate",
    "invoke",
    "search",
    "find",
    "make-series",
    "mv-expand",
    "mv-apply",
    "order",
    "parse",
    "datatable",
    "range",
    "facet",
    "fork",
    "partition",
    "scan",
    "lookup",
    "getschema",
    "externaldata",
    "materialize",
];

const KQL_OPERATORS: &[&str] = &[
    "and",
    "or",
    "not",
    "in",
    "!in",
    "contains",
    "!contains",
    "startswith",
    "!startswith",
    "endswith",
    "!endswith",
    "matches",
    "regex",
    "has",
    "!has",
    "hasprefix",
    "hassuffix",
    "contains_cs",
    "startswith_cs",
    "endswith_cs",
    "has_cs",
    "in~",
    "!in~",
    "has_any",
    "has_all",
    "between",
    "!between",
];

const KQL_FUNCTIONS: &[&str] = &[
    "ago",
    "now",
    "datetime",
    "timespan",
    "bin",
    "sum",
    "count",
    "avg",
    "min",
    "max",
    "dcount",
    "dcountif",
    "countif",
    "sumif",
    "avgif",
    "minif",
    "maxif",
    "stdev",
    "stdevif",
    "variance",
    "varianceif",
    "percentile",
    "percentiles",
    "make_list",
    "make_set",
    "make_bag",
    "arg_max",
    "arg_min",
    "any",
    "anyif",
    "tostring",
    "toint",
    "tolong",
    "todouble",
    "tobool",
    "todatetime",
    "totimespan",
    "strlen",
    "substring",
    "strcat",
    "split",
    "replace",
    "trim",
    "toupper",
    "tolower",
    "parse_json",
    "parse_xml",
    "parse_csv",
    "parse_url",
    "extract",
    "extract_all",
    "extractjson",
    "bag_keys",
    "bag_remove_keys",
    "pack",
    "pack_all",
    "pack_array",
    "todynamic",
    "array_length",
    "array_concat",
    "array_split",
    "set_union",
    "set_intersect",
    "set_difference",
    "iif",
    "iff",
    "case",
    "coalesce",
    "isempty",
    "isnotempty",
    "isnull",
    "isnotnull",
    "array_index_of",
    "hash",
    "format_datetime",
    "format_timespan",
    "dayofweek",
    "dayofmonth",
    "dayofyear",
    "week_of_year",
    "monthofyear",
    "getyear",
    "getmonth",
    "startofday",
    "startofweek",
    "startofmonth",
    "startofyear",
    "endofday",
    "endofweek",
    "endofmonth",
    "endofyear",
    "hourofday",
    "minuteofhour",
    "secondofminute",
];

const KQL_TYPES: &[&str] = &[
    "string", "int", "long", "real", "double", "bool", "datetime", "timespan", "dynamic", "guid",
    "decimal",
];

/// Token type for KQL syntax
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenType {
    Keyword,
    Operator,
    Function,
    Type,
    String,
    Number,
    Comment,
    Pipe,
    Punctuation,
    Variable,  // let bindings and references
    TableName, // Table/entity references (capitalized identifiers)
    Property,  // Field/column names
    Text,
}

/// Simple tokenizer for KQL
struct KqlTokenizer<'a> {
    input: &'a str,
    position: usize,
    last_token: Option<TokenType>,
    prev_word: Option<String>,
}

impl<'a> KqlTokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            position: 0,
            last_token: None,
            prev_word: None,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_while<F>(&mut self, predicate: F) -> &'a str
    where
        F: Fn(char) -> bool,
    {
        let start = self.position;
        while let Some(ch) = self.peek_char() {
            if predicate(ch) {
                self.advance();
            } else {
                break;
            }
        }
        &self.input[start..self.position]
    }

    /// Classify an identifier based on context and naming conventions
    fn classify_identifier(&self, word: &str) -> TokenType {
        // Check if it's after 'let' keyword (variable definition)
        if let Some(ref prev) = self.prev_word {
            if prev.to_lowercase() == "let" {
                return TokenType::Variable;
            }
        }

        // Check if it's after a pipe or at the start (likely a table name)
        if matches!(self.last_token, None | Some(TokenType::Pipe)) {
            // Table names are typically PascalCase or start with uppercase
            if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                return TokenType::TableName;
            }
        }

        // Check if followed by '(' - it's a function call
        let remaining = &self.input[self.position..].trim_start();
        if remaining.starts_with('(') {
            return TokenType::Function;
        }

        // After 'by', 'project', 'extend' - likely properties/columns
        if let Some(ref prev) = self.prev_word {
            let prev_lower = prev.to_lowercase();
            if prev_lower == "by" || prev_lower == "project" || prev_lower == "extend" {
                return TokenType::Property;
            }
        }

        // Lowercase identifiers after operators are likely properties
        if matches!(self.last_token, Some(TokenType::Operator)) {
            return TokenType::Property;
        }

        // PascalCase likely indicates table names
        if word.chars().next().is_some_and(|c| c.is_uppercase())
            && word.chars().skip(1).any(|c| c.is_lowercase())
        {
            return TokenType::TableName;
        }

        // Default to variable for other identifiers
        TokenType::Variable
    }

    fn next_token(&mut self) -> Option<(TokenType, &'a str)> {
        self.skip_whitespace();

        let start = self.position;
        let ch = self.peek_char()?;

        // Comments
        if ch == '/' && self.input[self.position..].starts_with("//") {
            let comment = self.read_while(|c| c != '\n');
            return Some((TokenType::Comment, comment));
        }

        // Strings
        if ch == '"' {
            self.advance(); // Skip opening quote
            let _content = self.read_while(|c| c != '"');
            self.advance(); // Skip closing quote (if present)
            return Some((TokenType::String, &self.input[start..self.position]));
        }

        // Single-quoted strings
        if ch == '\'' {
            self.advance(); // Skip opening quote
            let _content = self.read_while(|c| c != '\'');
            self.advance(); // Skip closing quote (if present)
            return Some((TokenType::String, &self.input[start..self.position]));
        }

        // Pipe operator and semicolon (statement separators)
        if ch == '|' {
            self.advance();
            self.last_token = Some(TokenType::Pipe);
            return Some((TokenType::Pipe, "|"));
        }

        if ch == ';' {
            self.advance();
            self.last_token = Some(TokenType::Pipe);
            return Some((TokenType::Pipe, ";"));
        }

        // Numbers
        if ch.is_ascii_digit() {
            let num = self.read_while(|c| c.is_ascii_digit() || c == '.');
            return Some((TokenType::Number, num));
        }

        // Punctuation (excluding semicolon, handled above)
        if "(),[]:".contains(ch) {
            self.advance();
            self.last_token = Some(TokenType::Punctuation);
            return Some((TokenType::Punctuation, &self.input[start..self.position]));
        }

        // Operators and identifiers
        if ch.is_alphabetic() || ch == '_' || ch == '!' {
            let word = self.read_while(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '!');
            let word_lower = word.to_lowercase();

            let token_type = if KQL_KEYWORDS.contains(&word_lower.as_str()) {
                TokenType::Keyword
            } else if KQL_OPERATORS.contains(&word_lower.as_str()) {
                TokenType::Operator
            } else if KQL_FUNCTIONS.contains(&word_lower.as_str()) {
                TokenType::Function
            } else if KQL_TYPES.contains(&word_lower.as_str()) {
                TokenType::Type
            } else {
                // Context-aware classification for identifiers
                self.classify_identifier(word)
            };

            self.last_token = Some(token_type);
            self.prev_word = Some(word.to_string());

            return Some((token_type, word));
        }

        // Other operators
        if "=<>!+-*/%~".contains(ch) {
            let op = self.read_while(|c| "=<>!+-*/%~".contains(c));
            return Some((TokenType::Operator, op));
        }

        // Unknown character - treat as text
        self.advance();
        Some((TokenType::Text, &self.input[start..self.position]))
    }
}

/// Highlight a single line of KQL code
pub fn highlight_line(line: &str) -> Vec<Span<'_>> {
    let mut spans = Vec::new();
    let mut tokenizer = KqlTokenizer::new(line);
    let mut last_pos = 0;

    while let Some((token_type, token_str)) = tokenizer.next_token() {
        // Add any whitespace before this token
        if tokenizer.position - token_str.len() > last_pos {
            let whitespace = &line[last_pos..(tokenizer.position - token_str.len())];
            if !whitespace.is_empty() {
                spans.push(Span::raw(whitespace.to_string()));
            }
        }

        let style = match token_type {
            // VS Code Dark+ inspired colors
            TokenType::Keyword => Style::default().fg(Color::LightMagenta), // VS Code: #C586C0 (pinkish-purple)
            TokenType::Operator => Style::default().fg(Color::White), // VS Code operators are often white/light gray
            TokenType::Function => Style::default().fg(Color::LightYellow), // VS Code: #DCDCAA (pale yellow for functions)
            TokenType::Type => Style::default().fg(Color::Cyan), // VS Code: #4EC9B0 (teal/cyan for types)
            TokenType::String => Style::default().fg(Color::LightRed), // VS Code: #CE9178 (peachy/salmon for strings)
            TokenType::Number => Style::default().fg(Color::LightGreen), // VS Code: #B5CEA8 (pale green for numbers)
            TokenType::Comment => Style::default().fg(Color::Green), // VS Code: #6A9955 (green for comments)
            TokenType::Pipe => Style::default().fg(Color::White), // Pipe/semicolon as white like other operators
            TokenType::Punctuation => Style::default().fg(Color::White), // VS Code: punctuation is typically white
            TokenType::Variable => Style::default().fg(Color::LightBlue), // VS Code: #9CDCFE (light blue for variables)
            TokenType::TableName => Style::default().fg(Color::LightCyan), // VS Code: #4EC9B0 (teal for class/type names)
            TokenType::Property => Style::default().fg(Color::LightBlue), // VS Code: #9CDCFE (light blue for properties)
            TokenType::Text => Style::default().fg(Color::White),         // Default text color
        };

        spans.push(Span::styled(token_str.to_string(), style));
        last_pos = tokenizer.position;
    }

    // Add any remaining whitespace
    if last_pos < line.len() {
        spans.push(Span::raw(line[last_pos..].to_string()));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_highlighting() {
        let line = "let x = 5";
        let spans = highlight_line(line);
        assert!(spans.len() >= 3);
    }

    #[test]
    fn test_pipe_highlighting() {
        let line = "table | where x > 5";
        let spans = highlight_line(line);
        assert!(spans.iter().any(|s| s.content == "|"));
    }

    #[test]
    fn test_string_highlighting() {
        let line = r#"where name == "test""#;
        let spans = highlight_line(line);
        assert!(!spans.is_empty());
    }
}
