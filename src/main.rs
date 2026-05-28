use flate2::read::GzDecoder;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::io::{Read, Write};

const COMPRESSED_WORDS: &[u8] = include_bytes!("words.txt.gz");
const COMPRESSED_FREQS: &[u8] = include_bytes!("freqs.bin.gz");

#[derive(Clone, Copy, Debug)]
enum RangeOrExact {
    Exact(usize),
    Range(usize, usize),
}

impl RangeOrExact {
    fn parse(s: &str) -> Result<Self, String> {
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

    fn resolve(self, rng: &mut ChaCha8Rng) -> usize {
        match self {
            RangeOrExact::Exact(val) => val,
            RangeOrExact::Range(min, max) => rng.random_range(min..=max),
        }
    }
}

fn decompress(bytes: &[u8]) -> Vec<u8> {
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("Failed to decompress embedded database asset");
    decompressed
}

fn print_help() {
    println!("Dutch Random Text Generator");
    println!("Generates pseudo-Dutch text statistically representative of the Dutch vocabulary.");
    println!();
    println!("Usage: dutch-gen [options]");
    println!();
    println!("Options:");
    println!(
        "  -w, --words <exact|min-max>   Generate text with exact words or range of words (e.g., 500 or 100-500)"
    );
    println!(
        "  -b, --bytes <exact|min-max>   Generate text with exact bytes or range of bytes (e.g., 1000 or 500-1500)"
    );
    println!("  -s, --seed <u64>              Specify seed for reproducible output");
    println!("  -h, --help                    Print this help menu");
}

struct CliArgs {
    words: Option<RangeOrExact>,
    bytes: Option<RangeOrExact>,
    seed: Option<u64>,
}

fn parse_args() -> CliArgs {
    let mut args = std::env::args().skip(1);
    let mut cli_words = None;
    let mut cli_bytes = None;
    let mut cli_seed = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-w" | "--words" => {
                if let Some(val_str) = args.next() {
                    match RangeOrExact::parse(&val_str) {
                        Ok(val) => cli_words = Some(val),
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Error: Missing value for {arg}");
                    std::process::exit(1);
                }
            }
            "-b" | "--bytes" => {
                if let Some(val_str) = args.next() {
                    match RangeOrExact::parse(&val_str) {
                        Ok(val) => cli_bytes = Some(val),
                        Err(e) => {
                            eprintln!("Error: {e}");
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Error: Missing value for {arg}");
                    std::process::exit(1);
                }
            }
            "-s" | "--seed" => {
                if let Some(val_str) = args.next() {
                    if let Ok(val) = val_str.parse::<u64>() {
                        cli_seed = Some(val);
                    } else {
                        eprintln!("Error: Invalid seed value: {val_str}");
                        std::process::exit(1);
                    }
                } else {
                    eprintln!("Error: Missing value for {arg}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Error: Unknown argument: {arg}");
                print_help();
                std::process::exit(1);
            }
        }
    }

    if cli_words.is_some() && cli_bytes.is_some() {
        eprintln!("Error: --words and --bytes parameters are mutually exclusive.");
        std::process::exit(1);
    }

    CliArgs {
        words: cli_words,
        bytes: cli_bytes,
        seed: cli_seed,
    }
}

fn sample_word<'a>(rng: &mut ChaCha8Rng, words: &[&'a str], cdf: &[u64], sum: u64) -> &'a str {
    let r = rng.random_range(1..=sum);
    let idx = cdf.binary_search(&r).unwrap_or_else(|i| i);
    words[idx]
}

fn generate_by_bytes(
    target_bytes: usize,
    rng: &mut ChaCha8Rng,
    words: &[&str],
    cdf: &[u64],
    sum: u64,
    writer: &mut impl Write,
) {
    let mut bytes_generated = 0;
    let mut bytes_written = 0;
    let mut first_paragraph = true;

    while bytes_generated < target_bytes {
        let paragraph_sentences_count = rng.random_range(3..=8);
        let mut sentences = Vec::new();

        for _ in 0..paragraph_sentences_count {
            if bytes_generated >= target_bytes {
                break;
            }

            let sentence_len = rng.random_range(5..=20);
            let mut sentence_words = Vec::with_capacity(sentence_len);
            for i in 0..sentence_len {
                let mut word = sample_word(rng, words, cdf, sum).to_string();
                if i == 0 {
                    let mut chars = word.chars();
                    word = match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    };
                }
                sentence_words.push(word);
            }

            let p = rng.random_range(0..100);
            let punct = if p < 85 {
                "."
            } else if p < 95 {
                "?"
            } else {
                "!"
            };
            let mut sentence_str = sentence_words.join(" ");
            sentence_str.push_str(punct);

            let added_len = sentence_str.len() + usize::from(!sentences.is_empty());
            bytes_generated += added_len;
            sentences.push(sentence_str);
        }

        if !sentences.is_empty() {
            let paragraph = sentences.join(" ");
            let chunk = if first_paragraph {
                paragraph
            } else {
                bytes_generated += 2;
                format!("\n\n{paragraph}")
            };

            if bytes_written + chunk.len() <= target_bytes {
                writer
                    .write_all(chunk.as_bytes())
                    .expect("Failed to write to stdout");
                bytes_written += chunk.len();
                first_paragraph = false;
            } else {
                let limit = target_bytes - bytes_written;
                let mut end = limit;
                while !chunk.is_char_boundary(end) {
                    end -= 1;
                }
                writer
                    .write_all(&chunk.as_bytes()[..end])
                    .expect("Failed to write to stdout");
                break;
            }
        }
    }
}

fn generate_by_words(
    target_words: usize,
    rng: &mut ChaCha8Rng,
    words: &[&str],
    cdf: &[u64],
    sum: u64,
    writer: &mut impl Write,
) {
    let mut words_generated = 0;
    let mut first_paragraph = true;

    while words_generated < target_words {
        let paragraph_sentences_count = rng.random_range(3..=8);
        let mut sentences = Vec::new();

        for _ in 0..paragraph_sentences_count {
            if words_generated >= target_words {
                break;
            }

            let remaining = target_words - words_generated;
            let sentence_len = if remaining <= 5 {
                remaining
            } else {
                rng.random_range(5..=20).min(remaining)
            };

            let mut sentence_words = Vec::with_capacity(sentence_len);
            for i in 0..sentence_len {
                let mut word = sample_word(rng, words, cdf, sum).to_string();
                if i == 0 {
                    let mut chars = word.chars();
                    word = match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    };
                }
                sentence_words.push(word);
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
            let mut sentence_str = sentence_words.join(" ");
            sentence_str.push_str(punct);
            sentences.push(sentence_str);
        }

        if !sentences.is_empty() {
            if first_paragraph {
                first_paragraph = false;
            } else {
                writer
                    .write_all(b"\n\n")
                    .expect("Failed to write to stdout");
            }
            writer
                .write_all(sentences.join(" ").as_bytes())
                .expect("Failed to write to stdout");
        }
    }
}

fn main() {
    let cli_args = parse_args();

    // 2. Initialize Seedable RNG
    let mut rng = match cli_args.seed {
        Some(s) => ChaCha8Rng::seed_from_u64(s),
        None => ChaCha8Rng::from_rng(&mut rand::rng()),
    };

    // 3. Decompress and parse embedded assets
    let words_raw = decompress(COMPRESSED_WORDS);
    let words_str = String::from_utf8(words_raw).expect("Words database is not valid UTF-8");
    let words: Vec<&str> = words_str.lines().collect();

    let freqs_raw = decompress(COMPRESSED_FREQS);
    let freqs: Vec<u32> = freqs_raw
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    if words.is_empty() {
        eprintln!("Error: Word database is empty.");
        std::process::exit(1);
    }
    assert_eq!(words.len(), freqs.len(), "Database file structure error");

    // 4. Build Cumulative Distribution Function (CDF)
    let mut cdf = Vec::with_capacity(freqs.len());
    let mut sum: u64 = 0;
    for &freq in &freqs {
        sum += u64::from(freq);
        cdf.push(sum);
    }

    // 5. Generate output based on parameters
    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());

    if let Some(bytes_range) = cli_args.bytes {
        let target_bytes = bytes_range.resolve(&mut rng);
        generate_by_bytes(target_bytes, &mut rng, &words, &cdf, sum, &mut writer);
    } else {
        let target_words = cli_args
            .words
            .unwrap_or(RangeOrExact::Exact(100))
            .resolve(&mut rng);
        generate_by_words(target_words, &mut rng, &words, &cdf, sum, &mut writer);
    }

    writer.write_all(b"\n").expect("Failed to write to stdout");
    writer.flush().expect("Failed to flush stdout");
}
