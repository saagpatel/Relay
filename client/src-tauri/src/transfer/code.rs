use rand::RngExt;

use crate::error::{AppError, AppResult};

const WORDLIST: &str = include_str!("../../../wordlist.txt");

/// Largest nameplate a code may carry. The server allocates the smallest free
/// number, so real nameplates stay short; this only bounds parsing.
pub const MAX_NAMEPLATE: u32 = 99_999;

/// A human-friendly transfer code: "{nameplate}-{word}-{word}".
///
/// The nameplate is the room number the signaling server allocated; it is
/// the only part the server ever sees. The two words are generated locally
/// and never leave the peers. The whole code string is the SPAKE2 password.
#[derive(Debug, Clone)]
pub struct TransferCode {
    pub nameplate: u32,
    pub word1: String,
    pub word2: String,
}

impl TransferCode {
    /// Generate fresh password words for a nameplate the server allocated.
    pub fn generate(nameplate: u32) -> Self {
        let mut rng = rand::rng();
        let words = wordlist();
        let word1 = words[rng.random_range(0..words.len())].to_string();
        let word2 = words[rng.random_range(0..words.len())].to_string();
        Self {
            nameplate,
            word1,
            word2,
        }
    }

    /// Format as "7-guitar-palace"
    pub fn to_code_string(&self) -> String {
        format!("{}-{}-{}", self.nameplate, self.word1, self.word2)
    }

    /// The room name to send to the signaling server: the nameplate alone.
    pub fn room(&self) -> String {
        self.nameplate.to_string()
    }

    /// Parse a code string like "7-guitar-palace"
    pub fn parse(code: &str) -> AppResult<Self> {
        let parts: Vec<&str> = code.trim().splitn(3, '-').collect();
        if parts.len() != 3 {
            return Err(AppError::InvalidCode(
                "expected format: number-word-word".into(),
            ));
        }

        let nameplate = parse_nameplate(parts[0])?;

        let words = wordlist();
        let word1 = parts[1].to_lowercase();
        let word2 = parts[2].to_lowercase();

        if !words.contains(&word1.as_str()) {
            return Err(AppError::InvalidCode(format!("unknown word: '{word1}'")));
        }
        if !words.contains(&word2.as_str()) {
            return Err(AppError::InvalidCode(format!("unknown word: '{word2}'")));
        }

        Ok(Self {
            nameplate,
            word1,
            word2,
        })
    }
}

/// Parse a nameplate: 1 to 5 ASCII digits, value 1..=MAX_NAMEPLATE.
pub fn parse_nameplate(value: &str) -> AppResult<u32> {
    let invalid = || AppError::InvalidCode(format!("nameplate must be a number 1-{MAX_NAMEPLATE}"));
    if value.is_empty() || value.len() > 5 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    let nameplate: u32 = value.parse().map_err(|_| invalid())?;
    if nameplate == 0 || nameplate > MAX_NAMEPLATE {
        return Err(invalid());
    }
    Ok(nameplate)
}

impl std::fmt::Display for TransferCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_code_string())
    }
}

fn wordlist() -> Vec<&'static str> {
    WORDLIST
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wordlist_has_256_words() {
        let words = wordlist();
        assert_eq!(words.len(), 256, "wordlist must contain exactly 256 words");
    }

    #[test]
    fn test_wordlist_no_duplicates() {
        let words = wordlist();
        let mut seen = std::collections::HashSet::new();
        for w in &words {
            assert!(seen.insert(w), "duplicate word: {w}");
        }
    }

    #[test]
    fn test_generate_and_parse_roundtrip() {
        for n in 1..50 {
            let code = TransferCode::generate(n * 37);
            let s = code.to_code_string();
            let parsed = TransferCode::parse(&s).unwrap();
            assert_eq!(parsed.nameplate, code.nameplate);
            assert_eq!(parsed.word1, code.word1);
            assert_eq!(parsed.word2, code.word2);
        }
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(TransferCode::parse("invalid").is_err());
        assert!(TransferCode::parse("abc-guitar-palace").is_err());
        assert!(TransferCode::parse("7-notaword-palace").is_err());
        assert!(TransferCode::parse("0-guitar-palace").is_err());
        assert!(TransferCode::parse("100000-guitar-palace").is_err());
        assert!(TransferCode::parse("+7-guitar-palace").is_err());
    }

    #[test]
    fn room_is_the_nameplate_alone() {
        let code = TransferCode::parse("42-guitar-tiger").unwrap();
        assert_eq!(code.room(), "42");
        assert!(!code.room().contains("guitar"));
        assert_eq!(code.to_code_string(), "42-guitar-tiger");
    }
}
