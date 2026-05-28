use flate2::read::GzDecoder;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use std::io::{Read, Write};

#[derive(Clone, Copy, Debug)]
pub enum RangeOrExact {
    Exact(usize),
    Range(usize, usize),
}

impl RangeOrExact {
    /// Parses a command line value as exact or range.
    ///
    /// # Errors
    /// Returns a string describing the parse error if formatting is invalid.
    pub fn parse(s: &str) -> Result<Self, String> {
        if let Some((min_str, max_str)) = s.split_once('-') {
            let min = min_str
                .parse::<usize>()
                .map_err(|_| format!("Invalid min value: {min_str}"))?;
            let max = max_str
                .parse::<usize>()
                .map_err(|_| format!("Invalid max value: {max_str}"))?;
            if min > max {
                return Err(format!("Min value {min} is greater than max value {max}"));
            }
            Ok(RangeOrExact::Range(min, max))
        } else {
            let val = s
                .parse::<usize>()
                .map_err(|_| format!("Invalid value: {s}"))?;
            Ok(RangeOrExact::Exact(val))
        }
    }

    #[must_use]
    pub fn resolve(self, rng: &mut ChaCha8Rng) -> usize {
        match self {
            RangeOrExact::Exact(val) => val,
            RangeOrExact::Range(min, max) => rng.random_range(min..=max),
        }
    }
}

/// Decompresses gzip-compressed bytes.
///
/// # Panics
/// Panics if gzip decompression fails (should not happen with embedded valid assets).
#[must_use]
pub fn decompress(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("Failed to decompress embedded database asset");
    decompressed
}

/// Samples a word using cumulative distribution function (CDF).
#[must_use]
pub fn sample_word<'a>(rng: &mut ChaCha8Rng, words: &[&'a str], cdf: &[u64], sum: u64) -> &'a str {
    let r = rng.random_range(1..=sum);
    let idx = cdf.binary_search(&r).unwrap_or_else(|i| i);
    words[idx]
}

/// Generates text with a target size in bytes.
///
/// # Errors
/// Returns an error if writing to the output stream fails.
pub fn generate_by_bytes(
    target_bytes: usize,
    rng: &mut ChaCha8Rng,
    words: &[&str],
    cdf: &[u64],
    sum: u64,
    writer: &mut impl Write,
) -> std::io::Result<()> {
    let mut bytes_written = 0;
    let mut first_paragraph = true;
    let mut paragraph_buf = String::with_capacity(2048);

    while bytes_written < target_bytes {
        paragraph_buf.clear();
        let paragraph_sentences_count = rng.random_range(3..=8);

        for s_idx in 0..paragraph_sentences_count {
            if bytes_written + paragraph_buf.len() >= target_bytes {
                break;
            }

            if s_idx > 0 {
                paragraph_buf.push(' ');
            }

            let sentence_len = rng.random_range(5..=20);
            for i in 0..sentence_len {
                if i > 0 {
                    paragraph_buf.push(' ');
                }
                let word = sample_word(rng, words, cdf, sum);
                if i == 0 {
                    let mut chars = word.chars();
                    if let Some(first) = chars.next() {
                        for c in first.to_uppercase() {
                            paragraph_buf.push(c);
                        }
                        paragraph_buf.push_str(chars.as_str());
                    }
                } else {
                    paragraph_buf.push_str(word);
                }
            }

            let p = rng.random_range(0..100);
            let punct = if p < 85 {
                "."
            } else if p < 95 {
                "?"
            } else {
                "!"
            };
            paragraph_buf.push_str(punct);
        }

        if !paragraph_buf.is_empty() {
            if first_paragraph {
                first_paragraph = false;
                if paragraph_buf.len() <= target_bytes {
                    writer.write_all(paragraph_buf.as_bytes())?;
                    bytes_written += paragraph_buf.len();
                } else {
                    let mut end = target_bytes;
                    while !paragraph_buf.is_char_boundary(end) {
                        end -= 1;
                    }
                    writer.write_all(&paragraph_buf.as_bytes()[..end])?;
                    break;
                }
            } else {
                // We need to write "\n\n" first.
                if bytes_written + 2 <= target_bytes {
                    writer.write_all(b"\n\n")?;
                    bytes_written += 2;

                    let remaining = target_bytes - bytes_written;
                    if paragraph_buf.len() <= remaining {
                        writer.write_all(paragraph_buf.as_bytes())?;
                        bytes_written += paragraph_buf.len();
                    } else {
                        let mut end = remaining;
                        while !paragraph_buf.is_char_boundary(end) {
                            end -= 1;
                        }
                        writer.write_all(&paragraph_buf.as_bytes()[..end])?;
                        break;
                    }
                } else {
                    // Not enough space even for "\n\n" or just 1 byte of it
                    let limit = target_bytes - bytes_written;
                    writer.write_all(&b"\n\n"[..limit])?;
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Generates text with a target count of words.
///
/// # Errors
/// Returns an error if writing to the output stream fails.
pub fn generate_by_words(
    target_words: usize,
    rng: &mut ChaCha8Rng,
    words: &[&str],
    cdf: &[u64],
    sum: u64,
    writer: &mut impl Write,
) -> std::io::Result<()> {
    let mut words_generated = 0;
    let mut first_paragraph = true;

    while words_generated < target_words {
        if first_paragraph {
            first_paragraph = false;
        } else {
            writer.write_all(b"\n\n")?;
        }

        let paragraph_sentences_count = rng.random_range(3..=8);
        let mut first_sentence_in_para = true;

        for _ in 0..paragraph_sentences_count {
            if words_generated >= target_words {
                break;
            }

            if first_sentence_in_para {
                first_sentence_in_para = false;
            } else {
                writer.write_all(b" ")?;
            }

            let remaining = target_words - words_generated;
            let sentence_len = if remaining <= 5 {
                remaining
            } else {
                rng.random_range(5..=20).min(remaining)
            };

            for i in 0..sentence_len {
                if i > 0 {
                    writer.write_all(b" ")?;
                }
                let word = sample_word(rng, words, cdf, sum);
                if i == 0 {
                    let mut chars = word.chars();
                    if let Some(first) = chars.next() {
                        for c in first.to_uppercase() {
                            write!(writer, "{c}")?;
                        }
                        writer.write_all(chars.as_str().as_bytes())?;
                    }
                } else {
                    writer.write_all(word.as_bytes())?;
                }
            }

            words_generated += sentence_len;

            let p = rng.random_range(0..100);
            let punct = if p < 85 {
                "."
            } else if p < 95 {
                "?"
            } else {
                "!"
            };
            writer.write_all(punct.as_bytes())?;
        }
    }
    Ok(())
}

/// Generates text infinitely.
///
/// # Errors
/// Returns an error if writing or flushing to the output stream fails.
pub fn generate_infinite(
    rng: &mut ChaCha8Rng,
    words: &[&str],
    cdf: &[u64],
    sum: u64,
    writer: &mut impl Write,
) -> std::io::Result<()> {
    let mut first_paragraph = true;
    loop {
        if first_paragraph {
            first_paragraph = false;
        } else {
            writer.write_all(b"\n\n")?;
        }

        let paragraph_sentences_count = rng.random_range(3..=8);
        for s_idx in 0..paragraph_sentences_count {
            if s_idx > 0 {
                writer.write_all(b" ")?;
            }

            let sentence_len = rng.random_range(5..=20);
            for i in 0..sentence_len {
                if i > 0 {
                    writer.write_all(b" ")?;
                }
                let word = sample_word(rng, words, cdf, sum);
                if i == 0 {
                    let mut chars = word.chars();
                    if let Some(first) = chars.next() {
                        for c in first.to_uppercase() {
                            write!(writer, "{c}")?;
                        }
                        writer.write_all(chars.as_str().as_bytes())?;
                    }
                } else {
                    writer.write_all(word.as_bytes())?;
                }
            }

            let p = rng.random_range(0..100);
            let punct = if p < 85 {
                "."
            } else if p < 95 {
                "?"
            } else {
                "!"
            };
            writer.write_all(punct.as_bytes())?;
        }

        writer.flush()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_range_or_exact_parse_exact() {
        let parsed = RangeOrExact::parse("123").unwrap();
        assert!(matches!(parsed, RangeOrExact::Exact(123)));
    }

    #[test]
    fn test_range_or_exact_parse_range() {
        let parsed = RangeOrExact::parse("10-20").unwrap();
        assert!(matches!(parsed, RangeOrExact::Range(10, 20)));
    }

    #[test]
    fn test_range_or_exact_parse_invalid_format() {
        assert!(RangeOrExact::parse("abc").is_err());
        assert!(RangeOrExact::parse("10-abc").is_err());
        assert!(RangeOrExact::parse("abc-20").is_err());
    }

    #[test]
    fn test_range_or_exact_parse_min_greater_than_max() {
        let res = RangeOrExact::parse("20-10");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            "Min value 20 is greater than max value 10"
        );
    }

    #[test]
    fn test_generate_by_words_basic() {
        let words = vec!["hallo", "wereld", "test"];
        let cdf = vec![1, 2, 3];
        let sum = 3;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut output = Vec::new();

        generate_by_words(10, &mut rng, &words, &cdf, sum, &mut output).unwrap();

        let generated_str = String::from_utf8(output).unwrap();
        assert!(!generated_str.is_empty());
        let word_count = generated_str.split_whitespace().count();
        assert_eq!(word_count, 10);
    }

    #[test]
    fn test_generate_by_bytes_basic() {
        let words = vec!["een", "twee", "drie"];
        let cdf = vec![1, 2, 3];
        let sum = 3;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut output = Vec::new();

        generate_by_bytes(50, &mut rng, &words, &cdf, sum, &mut output).unwrap();

        let generated_str = String::from_utf8(output).unwrap();
        assert_eq!(generated_str.len(), 50);
    }
}
