use log::{debug, trace};
use tower_lsp_server::lsp_types::{Position, Range, TextEdit, Uri};

#[derive(Debug)]
pub struct SystemdFormatter;

impl SystemdFormatter {
    pub fn new() -> Self {
        Self
    }

    pub fn format_document(&self, uri: &Uri, text: &str) -> Vec<TextEdit> {
        debug!("Formatting document: {:?}", uri);
        trace!("Document text length: {}", text.len());

        let formatted_content = self.apply_opinionated_formatting(text);

        // If content changed, replace the entire document
        if formatted_content != text {
            vec![TextEdit {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: text.lines().count() as u32,
                        character: 0,
                    },
                },
                new_text: formatted_content,
            }]
        } else {
            vec![]
        }
    }

    pub fn format_range(&self, uri: &Uri, text: &str, _range: Range) -> Vec<TextEdit> {
        debug!("Formatting range in document: {:?}", uri);

        // For range formatting, we'll format the entire document to maintain consistency
        // since our opinionated formatting affects spacing between sections
        self.format_document(uri, text)
    }

    fn apply_opinionated_formatting(&self, content: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        // let mut in_section = false;
        // let mut previous_was_section = false;

        // let mut is_continued_line = false; // would need to track line wraps and carry them across comment lines too
        for line in lines.iter() {
            // print!("{}", line.to_string());

            let is_blank = line.chars().all(|c| c.is_whitespace());
            if is_blank {
                // DO NOT SKIP BLANKS... if you have a line continuation and you skip the blank line after the final line... oh boy... you end up merging the next key=value line with the line continuation
                // if you leave a blank line then you can always have a trailing \ on the end of every component of a large command (so y ou can rearrange args)
                //  but strip those blanks and KABOOM
                result.push("".to_string()); // insert blank
                continue;
            }

            // use pattern matching instead of trim
            let trimmed = line.trim();

            // comments - only trim trailing whitespace
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                result.push(line.trim_end().to_string());
                continue;
            }

            // Handle section headers
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                // Add single blank line before section (except for first section)
                // if in_section || previous_was_section {
                //     result.push(String::new());
                // }

                // allow trim start/end of section headers
                result.push(trimmed.to_string());
                // in_section = true;
                // previous_was_section = true;
                continue;
            }

            // Handle directives (key=value pairs)
            if let Some(equals_pos) = trimmed.find('=') {
                let key = trimmed[..equals_pos].trim();
                let value = trimmed[equals_pos + 1..].trim();

                // Opinionated formatting: no spaces around equals
                let formatted = format!("{}={}", key, value);
                result.push(formatted);
                // previous_was_section = false;
                continue;
            }

            // Handle any other lines (preserve leading indentation, trim trailing)
            // - currently should only be continuation lines
            // - and any unrecognized lines (i.e. if new syntax in future)
            result.push(line.trim_end().to_string());
            // previous_was_section = false;
        }

        // Join with newlines and ensure file ends with single newline
        let mut formatted = result.join("\n");
        if !formatted.is_empty() && !formatted.ends_with('\n') {
            formatted.push('\n');
        }

        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keep_indent_on_line_wrap() {
        let formatter = SystemdFormatter::new();
        let input = "ExecStart=\\\n    /usr/bin/foo\n";
        let expected = "ExecStart=\\\n    /usr/bin/foo\n";
        let formatted = formatter.apply_opinionated_formatting(input);
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_opinionated_formatting_no_spaces_around_equals() {
        let formatter = SystemdFormatter::new();
        let input = "[Unit]\nDescription = Test Service\nAfter =  network.target\nWants=   network-online.target\n";
        let expected =
            "[Unit]\nDescription=Test Service\nAfter=network.target\nWants=network-online.target\n";

        let formatted = formatter.apply_opinionated_formatting(input);
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_comments_strip_trailing_whitespace() {
        let formatter = SystemdFormatter::new();
        let input = "# comment with whitespace after \n";
        let expected = "# comment with whitespace after\n";

        let formatted = formatter.apply_opinionated_formatting(input);
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_comments_preserve_leading_whitespace() {
        let formatter = SystemdFormatter::new();
        // both comment chars `;` and `#`
        let input = "  ; indented comment\n  # indented comment\n";
        let expected = "  ; indented comment\n  # indented comment\n";

        let formatted = formatter.apply_opinionated_formatting(input);
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_leave_all_blank_lines() {
        let formatter = SystemdFormatter::new();
        let input = "\n\n[Unit]\n\n\nDescription=Test\n";
        let expected = "\n\n[Unit]\n\n\nDescription=Test\n";

        let formatted = formatter.apply_opinionated_formatting(input);
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_opinionated_formatting_single_section() {
        let formatter = SystemdFormatter::new();
        let input = "[Unit]\nDescription=Test\nAfter=network.target\n";
        let expected = "[Unit]\nDescription=Test\nAfter=network.target\n";

        let formatted = formatter.apply_opinionated_formatting(input);
        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_document_integration() {
        let formatter = SystemdFormatter::new();
        let uri = "file:///test.service".parse::<Uri>().unwrap();
        let input = "[Unit]\nDescription = Test\n[Service]\nType = simple\n";

        let edits = formatter.format_document(&uri, input);
        assert_eq!(edits.len(), 1);

        let expected = "[Unit]\nDescription=Test\n[Service]\nType=simple\n";
        assert_eq!(edits[0].new_text, expected);
    }

    #[test]
    fn test_format_document_no_changes_needed() {
        let formatter = SystemdFormatter::new();
        let uri = "file:///test.service".parse::<Uri>().unwrap();
        let input = "[Unit]\nDescription=Test\n\n[Service]\nType=simple\n";

        let edits = formatter.format_document(&uri, input);
        assert_eq!(edits.len(), 0);
    }

}
