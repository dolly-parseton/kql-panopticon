//! ANSI escape code parser
//!
//! Converts text with ANSI color codes to ratatui styled spans.
//! Supports basic SGR (Select Graphic Rendition) codes for colors.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Parse text containing ANSI escape codes into styled spans
pub fn parse_ansi(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_style = Style::default();
    let mut current_text = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Start of escape sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['

                // Flush current text
                if !current_text.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut current_text),
                        current_style,
                    ));
                }

                // Parse SGR sequence
                let mut code = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit() || ch == ';' {
                        code.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                // Check for 'm' (SGR terminator)
                if chars.peek() == Some(&'m') {
                    chars.next(); // consume 'm'
                    current_style = apply_sgr_codes(&code, current_style);
                }
                // Other escape sequences are ignored
            } else {
                // Not a CSI sequence, output as-is
                current_text.push(c);
            }
        } else {
            current_text.push(c);
        }
    }

    // Flush remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    spans
}

/// Apply SGR (Select Graphic Rendition) codes to style
fn apply_sgr_codes(codes: &str, mut style: Style) -> Style {
    if codes.is_empty() || codes == "0" {
        // Reset
        return Style::default();
    }

    for code in codes.split(';') {
        match code {
            "0" => style = Style::default(),
            "1" => style = style.add_modifier(Modifier::BOLD),
            "2" => style = style.add_modifier(Modifier::DIM),
            "3" => style = style.add_modifier(Modifier::ITALIC),
            "4" => style = style.add_modifier(Modifier::UNDERLINED),
            // Foreground colors (30-37)
            "30" => style = style.fg(Color::Black),
            "31" => style = style.fg(Color::Red),
            "32" => style = style.fg(Color::Green),
            "33" => style = style.fg(Color::Yellow),
            "34" => style = style.fg(Color::Blue),
            "35" => style = style.fg(Color::Magenta),
            "36" => style = style.fg(Color::Cyan),
            "37" => style = style.fg(Color::White),
            "39" => style = style.fg(Color::Reset), // Default foreground
            // Bright foreground colors (90-97)
            "90" => style = style.fg(Color::DarkGray),
            "91" => style = style.fg(Color::LightRed),
            "92" => style = style.fg(Color::LightGreen),
            "93" => style = style.fg(Color::LightYellow),
            "94" => style = style.fg(Color::LightBlue),
            "95" => style = style.fg(Color::LightMagenta),
            "96" => style = style.fg(Color::LightCyan),
            "97" => style = style.fg(Color::White),
            // Background colors (40-47)
            "40" => style = style.bg(Color::Black),
            "41" => style = style.bg(Color::Red),
            "42" => style = style.bg(Color::Green),
            "43" => style = style.bg(Color::Yellow),
            "44" => style = style.bg(Color::Blue),
            "45" => style = style.bg(Color::Magenta),
            "46" => style = style.bg(Color::Cyan),
            "47" => style = style.bg(Color::White),
            "49" => style = style.bg(Color::Reset), // Default background
            _ => {} // Ignore unknown codes
        }
    }

    style
}

/// Parse a single line with ANSI codes into a ratatui Line
pub fn parse_ansi_line(text: &str) -> ratatui::text::Line<'static> {
    ratatui::text::Line::from(parse_ansi(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let spans = parse_ansi("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello world");
    }

    #[test]
    fn test_green_text() {
        let spans = parse_ansi("\x1b[32m✓\x1b[0m success");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "✓");
        assert_eq!(spans[0].style.fg, Some(Color::Green));
        assert_eq!(spans[1].content, " success");
    }

    #[test]
    fn test_red_text() {
        let spans = parse_ansi("\x1b[31m✗\x1b[0m error");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "✗");
        assert_eq!(spans[0].style.fg, Some(Color::Red));
    }

    #[test]
    fn test_yellow_text() {
        let spans = parse_ansi("\x1b[33m?\x1b[0m unknown");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "?");
        assert_eq!(spans[0].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_multiple_colors() {
        let spans = parse_ansi("\x1b[32m●\x1b[0m available  \x1b[33m●\x1b[0m stale");
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].style.fg, Some(Color::Green));
        assert_eq!(spans[2].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_reset_clears_style() {
        let spans = parse_ansi("\x1b[32mgreen\x1b[0mnormal");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].style.fg, Some(Color::Green));
        assert_eq!(spans[1].style.fg, None);
    }
}
