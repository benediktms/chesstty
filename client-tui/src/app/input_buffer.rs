use cozy_chess::Square;

/// Buffer for collecting square input (e.g., "e2", "e4")
#[derive(Debug, Clone, Default)]
pub struct InputBuffer {
    buffer: String,
}

impl InputBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn push_char(&mut self, c: char) {
        // Only accept valid chess notation characters
        if self.buffer.len() < 2 && (c.is_ascii_lowercase() || c.is_ascii_digit()) {
            self.buffer.push(c);
        }
    }

    pub fn backspace(&mut self) {
        self.buffer.pop();
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_complete(&self) -> bool {
        self.buffer.len() == 2
    }

    /// Try to parse the buffer as a square (e.g., "e2" -> Square)
    pub fn try_parse_square(&self) -> Option<Square> {
        if !self.is_complete() {
            return None;
        }

        let chars: Vec<char> = self.buffer.chars().collect();
        let file_char = chars[0];
        let rank_char = chars[1];

        // Parse file (a-h)
        let file = match file_char {
            'a' => cozy_chess::File::A,
            'b' => cozy_chess::File::B,
            'c' => cozy_chess::File::C,
            'd' => cozy_chess::File::D,
            'e' => cozy_chess::File::E,
            'f' => cozy_chess::File::F,
            'g' => cozy_chess::File::G,
            'h' => cozy_chess::File::H,
            _ => return None,
        };

        // Parse rank (1-8)
        let rank = match rank_char {
            '1' => cozy_chess::Rank::First,
            '2' => cozy_chess::Rank::Second,
            '3' => cozy_chess::Rank::Third,
            '4' => cozy_chess::Rank::Fourth,
            '5' => cozy_chess::Rank::Fifth,
            '6' => cozy_chess::Rank::Sixth,
            '7' => cozy_chess::Rank::Seventh,
            '8' => cozy_chess::Rank::Eighth,
            _ => return None,
        };

        Some(Square::new(file, rank))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_buffer() {
        let mut buffer = InputBuffer::new();
        assert!(buffer.is_empty());

        buffer.push_char('e');
        assert_eq!(buffer.as_str(), "e");
        assert!(!buffer.is_complete());

        buffer.push_char('2');
        assert_eq!(buffer.as_str(), "e2");
        assert!(buffer.is_complete());

        let square = buffer.try_parse_square();
        assert!(square.is_some());
    }

    #[test]
    fn test_backspace() {
        let mut buffer = InputBuffer::new();
        buffer.push_char('e');
        buffer.push_char('2');
        buffer.backspace();
        assert_eq!(buffer.as_str(), "e");
    }
}
