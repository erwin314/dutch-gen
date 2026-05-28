use flate2::read::GzDecoder;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use std::io::{Read, Write};
use std::fmt::Write as _;

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

struct PdfWriter {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfWriter {
    fn new() -> Self {
        let mut writer = PdfWriter {
            buf: Vec::new(),
            offsets: Vec::new(),
        };
        writer.buf.extend_from_slice(b"%PDF-1.4\n");
        writer
    }

    fn write_object(&mut self, id: usize, content: &[u8]) {
        while self.offsets.len() <= id {
            self.offsets.push(0);
        }
        self.offsets[id] = self.buf.len();
        let header = format!("{id} 0 obj\n");
        self.buf.extend_from_slice(header.as_bytes());
        self.buf.extend_from_slice(content);
        self.buf.extend_from_slice(b"\nendobj\n");
    }

    fn finish(mut self) -> Vec<u8> {
        let xref_start = self.buf.len();
        self.buf.extend_from_slice(b"xref\n");
        let xref_header = format!("0 {}\n", self.offsets.len());
        self.buf.extend_from_slice(xref_header.as_bytes());
        self.buf.extend_from_slice(b"0000000000 65535 f \n");
        for &offset in &self.offsets[1..] {
            let entry = format!("{offset:010} 00000 n \n");
            self.buf.extend_from_slice(entry.as_bytes());
        }
        self.buf.extend_from_slice(b"trailer\n");
        let trailer_dict = format!("<< /Size {} /Root 1 0 R >>\n", self.offsets.len());
        self.buf.extend_from_slice(trailer_dict.as_bytes());
        self.buf.extend_from_slice(b"startxref\n");
        let startxref = format!("{xref_start}\n");
        self.buf.extend_from_slice(startxref.as_bytes());
        self.buf.extend_from_slice(b"%%EOF\n");
        self.buf
    }
}

fn encode_cp1252_char(c: char) -> Option<u8> {
    let u = c as u32;
    if let Ok(b) = u8::try_from(u) {
        Some(b)
    } else {
        match c {
            '€' => Some(128),
            '‚' => Some(130),
            'ƒ' => Some(131),
            '„' => Some(132),
            '…' => Some(133),
            '†' => Some(134),
            '‡' => Some(135),
            'ˆ' => Some(136),
            '‰' => Some(137),
            'Š' => Some(138),
            '‹' => Some(139),
            'Œ' => Some(140),
            'Ž' => Some(142),
            '‘' => Some(145),
            '’' => Some(146),
            '“' => Some(147),
            '”' => Some(148),
            '•' => Some(149),
            '–' => Some(150),
            '—' => Some(151),
            '˜' => Some(152),
            '™' => Some(153),
            'š' => Some(154),
            '›' => Some(155),
            'œ' => Some(156),
            'ž' => Some(158),
            'Ÿ' => Some(159),
            _ => None,
        }
    }
}

fn escape_pdf_string(s: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for c in s.chars() {
        let b = encode_cp1252_char(c).unwrap_or(b'?');
        match b {
            b'(' => bytes.extend_from_slice(b"\\("),
            b')' => bytes.extend_from_slice(b"\\)"),
            b'\\' => bytes.extend_from_slice(b"\\\\"),
            _ => bytes.push(b),
        }
    }
    bytes
}

/// Generates a PDF document from plain text.
#[must_use]
pub fn generate_pdf(text: &str) -> Vec<u8> {
    // 1. Line wrapping (max 80 chars per line)
    let mut lines = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    for (i, &para) in paragraphs.iter().enumerate() {
        if i > 0 {
            lines.push(String::new()); // blank line
        }
        let words: Vec<&str> = para.split_whitespace().collect();
        let mut current_line = String::new();
        for &word in &words {
            if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_line.len() + 1 + word.len() <= 80 {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    // 2. Paginate (58 lines per page)
    let mut pages = Vec::new();
    let mut current_page = Vec::new();
    for line in lines {
        if current_page.len() >= 58 {
            pages.push(current_page);
            current_page = Vec::new();
        }
        // Avoid starting a page with an empty line
        if current_page.is_empty() && line.is_empty() {
            continue;
        }
        current_page.push(line);
    }
    if !current_page.is_empty() {
        pages.push(current_page);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }

    let num_pages = pages.len();
    let mut pdf = PdfWriter::new();

    // Catalog: Object 1
    pdf.write_object(1, b"<< /Type /Catalog /Pages 2 0 R >>");

    // Pages list: Object 2
    let mut kids = String::new();
    for i in 0..num_pages {
        let _ = write!(kids, "{} 0 R ", 3 + 2 * i);
    }
    let pages_content = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.trim_end(),
        num_pages
    );
    pdf.write_object(2, pages_content.as_bytes());

    // Font: Object 3 + 2 * num_pages
    let font_obj_id = 3 + 2 * num_pages;

    // Pages & Content streams
    for (i, page_lines) in pages.into_iter().enumerate() {
        let page_obj_id = 3 + 2 * i;
        let content_obj_id = 4 + 2 * i;

        // Write Page object
        let page_dict = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents {content_obj_id} 0 R /Resources << /Font << /F1 {font_obj_id} 0 R >> >> >>"
        );
        pdf.write_object(page_obj_id, page_dict.as_bytes());

        // Construct content stream (10pt font, 12.5pt leading, start at y=780)
        let mut stream_content = Vec::new();
        stream_content.extend_from_slice(b"BT\n/F1 10 Tf\n12.5 TL\n50 780 Td\n");
        for line in page_lines {
            let escaped = escape_pdf_string(&line);
            stream_content.extend_from_slice(b"(");
            stream_content.extend_from_slice(&escaped);
            stream_content.extend_from_slice(b") Tj T*\n");
        }
        stream_content.extend_from_slice(b"ET\n");

        let mut content_obj = format!("<< /Length {} >>\nstream\n", stream_content.len()).into_bytes();
        content_obj.extend_from_slice(&stream_content);
        content_obj.extend_from_slice(b"\nendstream");
        pdf.write_object(content_obj_id, &content_obj);
    }

    // Write Font object
    pdf.write_object(
        font_obj_id,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );

    pdf.finish()
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

    #[test]
    fn test_encode_cp1252() {
        assert_eq!(encode_cp1252_char('a'), Some(b'a'));
        assert_eq!(encode_cp1252_char('é'), Some(233));
        assert_eq!(encode_cp1252_char('ë'), Some(235));
        assert_eq!(encode_cp1252_char('€'), Some(128));
        assert_eq!(encode_cp1252_char('山'), None);
    }

    #[test]
    fn test_escape_pdf_string() {
        let escaped = escape_pdf_string("hallo (wereld) \\ test €");
        let mut expected = b"hallo \\(wereld\\) \\\\ test ".to_vec();
        expected.push(128);
        assert_eq!(escaped, expected);
    }

    #[test]
    fn test_pdf_generation_basic() {
        let text = "Dit is een test van de PDF generator.\n\nHet heeft meerdere paragrafen.";
        let pdf_data = generate_pdf(text);

        assert!(pdf_data.starts_with(b"%PDF-1.4"));
        assert!(pdf_data.ends_with(b"%%EOF\n"));

        let pdf_str = String::from_utf8_lossy(&pdf_data);
        assert!(pdf_str.contains("xref\n"));
        assert!(pdf_str.contains("trailer\n"));
        assert!(pdf_str.contains("startxref\n"));

        let xref_index = pdf_str.find("xref\n").unwrap();
        let trailer_index = pdf_str.find("trailer\n").unwrap();
        let xref_lines: Vec<&str> = pdf_str[xref_index..trailer_index]
            .lines()
            .skip(2)
            .collect();

        for line in xref_lines {
            assert_eq!(line.len(), 19);
        }
    }
}
