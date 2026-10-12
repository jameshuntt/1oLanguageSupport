// --- Line Map Implementation for Language Server Protocol (LSP) ---

use crate::engine::Span;
// Assuming Span is imported from crate::engine::Span.
// A minimal mock structure is defined in the test module for independent testing.
// #[cfg(not(test))]
// use crate::engine::Span;

use lsp_types::{Position, Range};

// --- Main Struct ---

/// A utility structure responsible for mapping absolute byte offsets within a text buffer
/// to the line-column coordinates (LSP Position) required by the Language Server Protocol (LSP).
///
/// This structure acts as a vital bridge between a compiler/parser, which uses byte indices,
/// and a text editor, which uses zero-based line and column numbers.
#[derive(Debug)]
pub struct LineMap {
    /// A vector containing the absolute byte offset where each line in the text buffer begins.
    ///
    /// - The first element is always 0 (the start of the file).
    /// - Subsequent elements mark the byte immediately following a newline character ('\n').
    pub line_starts: Vec<usize>,
}

impl LineMap {
    /// Constructs a new `LineMap` by analyzing the provided text buffer.
    ///
    /// It iterates through the text to find and record the byte offset of every line break.
    ///
    /// # Arguments
    /// * `text`: The full string content of the document.
    pub fn new(text: &str) -> Self {
        let mut line_starts = Vec::new();
        // Rule: The first line always starts at byte offset 0.
        line_starts.push(0);

        // Iterate over character indices to accurately find the byte offset of newlines.
        for (i, ch) in text.char_indices() {
            if ch == '\n' {
                // The next line begins one byte after the newline character.
                line_starts.push(i + 1);
            }
        }

        Self { line_starts }
    }

    /// Converts an absolute byte offset within the document into an LSP `Position`.
    ///
    /// The line index is zero-based. Character index is calculated from the start of the line.
    ///
    /// # Efficiency
    /// The line lookup is performed using binary search, making the complexity O(log n),
    /// where n is the number of lines, ensuring fast lookups even for large files.
    ///
    /// # Important LSP Compliance Note
    /// **WARNING:** The `character` field in the returned `Position` is calculated as the
    /// **byte offset** from the start of the line (`offset - line_start`). The LSP specification
    /// strictly requires the `character` field to be indexed using **UTF-16 code units**.
    ///
    /// If the source text contains any multi-byte UTF-8 characters (like emojis or non-Latin scripts),
    /// this byte-based calculation will be incorrect for LSP clients. To fix this, this method
    /// would require access to the original text slice to correctly count the UTF-16 code units.
    ///
    /// # Arguments
    /// * `offset`: The absolute byte offset within the document.
    ///
    /// # Returns
    /// An `lsp_types::Position` corresponding to the input byte offset.
    pub fn position(&self, offset: usize) -> Position {
        // Find the line number by performing a binary search on the `line_starts` array.
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            // If the offset is not a line start, `Err(next_line)` gives the index of
            // the first line start *after* the offset. Therefore, the current line index
            // is the one immediately preceding it (`next_line - 1`).
            Err(next_line) => next_line - 1,
        };

        // Get the absolute byte offset where the identified line begins.
        let line_start = self.line_starts[line];
        // Calculate the character index as the byte difference from the line start.
        // (This is the byte-based character offset, see WARNING above.)
        let character = offset - line_start;

        Position {
            // Line index is zero-based, cast to u32 for the LSP type.
            line: line as u32,
            // Character index (byte-based offset), cast to u32 for the LSP type.
            character: character as u32,
        }
    }

    /// Converts a compiler/parser `Span` (a pair of absolute byte offsets) into an
    /// LSP `Range` (a pair of Positions).
    ///
    /// This method simply calls `position` for both the start and end of the span.
    ///
    /// # Arguments
    /// * `span`: A span structure containing `start` and `end` absolute byte offsets.
    ///
    /// # Returns
    /// An `lsp_types::Range` covering the span.
    pub fn span_to_range(&self, span: Span) -> Range {
        Range {
            // Convert the start byte offset to an LSP Position.
            start: self.position(span.start),
            // Convert the end byte offset to an LSP Position.
            end: self.position(span.end),
        }
    }
}


// --- Test Module ---

#[cfg(test)]
mod tests {
    // Bring all items from the outer module into scope.
    use super::*;

    // Mock Span struct for testing purposes since it is defined in another module (`crate::engine::Span`).
    // #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    // pub struct Span {
    //     pub start: usize,
    //     pub end: usize,
    // }

    /// Test case for an empty input string to ensure the map initializes correctly.
    #[test]
    fn test_empty_text() {
        let text = "";
        let map = LineMap::new(text);
        // An empty file should still have one line start at offset 0.
        assert_eq!(map.line_starts, vec![0]);
    }

    /// Test case for single line text with no newline character.
    #[test]
    fn test_single_line() {
        let text = "Hello, LSP!";
        let map = LineMap::new(text);
        // Only one line start at the beginning.
        assert_eq!(map.line_starts, vec![0]);

        // Position at start (offset 0)
        assert_eq!(map.position(0), Position { line: 0, character: 0 });
        // Position in the middle (offset 7, char 7)
        assert_eq!(map.position(7), Position { line: 0, character: 7 });
        // Position at the end of the text (offset 11, char 11)
        assert_eq!(map.position(11), Position { line: 0, character: 11 });
    }

    /// Test case for multiple lines ending with a final newline character.
    #[test]
    fn test_multi_line_ending_newline() {
        // Text: "Line 1\nSecond\nThird\n" (Length 20)
        // Offsets: 0        6     7          13    14      19    20
        //          |L0 start| \n |L1 start| \n |L2 start| \n |L3 start (empty)|
        let text = "Line 1\nSecond\nThird\n";
        let map = LineMap::new(text);
        // Line starts should be at 0, 7, 14, and 20.
        assert_eq!(map.line_starts, vec![0, 7, 14, 20]);

        // Line 0: "Line 1" (offsets 0-6)
        assert_eq!(map.position(6), Position { line: 0, character: 6 }); // End of L0

        // Line 1: "Second" (offsets 7-13)
        assert_eq!(map.position(7), Position { line: 1, character: 0 }); // Start of L1
        assert_eq!(map.position(12), Position { line: 1, character: 5 }); // Middle of L1

        // Line 2: "Third" (offsets 14-19)
        assert_eq!(map.position(19), Position { line: 2, character: 5 }); // End of L2

        // Line 3: Empty line after final newline (offset 20)
        assert_eq!(map.position(20), Position { line: 3, character: 0 });
    }

    /// Test case for multi-line text not ending in a newline.
    #[test]
    fn test_multi_line_no_ending_newline() {
        // Text: "Line 1\nSecond\nThird" (Length 19)
        // Offsets: 0        6     7          13    14      18 19
        //          |L0 start| \n |L1 start| \n |L2 start| |  |
        let text = "Line 1\nSecond\nThird";
        let map = LineMap::new(text);
        // Line starts should be at 0, 7, and 14. No final line start is recorded.
        assert_eq!(map.line_starts, vec![0, 7, 14]);

        // Test position at the very end of the file.
        assert_eq!(map.position(19), Position { line: 2, character: 5 });
    }

    /// Test Span to Range conversion, including cross-line spans.
    #[test]
    fn test_span_to_range() {
        // Text: "Hello\nWorld\nRust"
        // Line Starts: [0, 6, 12]
        let text = "Hello\nWorld\nRust";
        let map = LineMap::new(text);

        // Span covering 'World' (offsets 6 to 11) -> Should be L1:C0 to L1:C5
        let span_mid = Span { start: 6, end: 11 };
        let range_mid = map.span_to_range(span_mid);
        assert_eq!(range_mid.start, Position { line: 1, character: 0 });
        assert_eq!(range_mid.end, Position { line: 1, character: 5 });

        // Span crossing lines (from 'o' in Hello (offset 4) to 'R' in Rust (offset 12))
        // Start: L0:C4
        // End: L2:C0
        let span_cross = Span { start: 4, end: 12 };
        let range_cross = map.span_to_range(span_cross);
        assert_eq!(range_cross.start, Position { line: 0, character: 4 });
        assert_eq!(range_cross.end, Position { line: 2, character: 0 });
    }
}




// use crate::engine::Span;
// use lsp_types::{Position, Range};
// 
// #[derive(Debug)]
// pub struct LineMap {
//     /// The byte offsets where each line begins.
//     line_starts: Vec<usize>,
// }
// 
// impl LineMap {
//     /// Build a line map for a given text buffer.
//     pub fn new(text: &str) -> Self {
//         let mut line_starts = Vec::new();
//         line_starts.push(0); // first line begins at offset 0
// 
//         for (i, ch) in text.char_indices() {
//             if ch == '\n' {
//                 line_starts.push(i + 1);
//             }
//         }
// 
//         Self { line_starts }
//     }
// 
//     /// Convert absolute byte offset → LSP Position
//     pub fn position(&self, offset: usize) -> Position {
//         // Find the line number by binary search
//         let line = match self.line_starts.binary_search(&offset) {
//             Ok(line) => line,
//             Err(next_line) => next_line - 1,
//         };
// 
//         let line_start = self.line_starts[line];
//         let character = offset - line_start;
// 
//         Position {
//             line: line as u32,
//             character: character as u32,
//         }
//     }
// 
//     /// Convert a Span → LSP Range
//     pub fn span_to_range(&self, span: Span) -> Range {
//         Range {
//             start: self.position(span.start),
//             end: self.position(span.end),
//         }
//     }
// }
// 