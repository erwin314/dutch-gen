use dutch_gen::{
    RangeOrExact, decompress, generate_by_bytes, generate_by_words, generate_infinite,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::io::Write;

const COMPRESSED_WORDS: &[u8] = include_bytes!("words.txt.gz");
const COMPRESSED_FREQS: &[u8] = include_bytes!("freqs.bin.gz");

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
    println!("  -i, --infinite                Generate text infinitely (streaming mode)");
    println!("  -s, --seed <u64>              Specify seed for reproducible output");
    println!("  -p, --pdf                     Generate output in PDF format");
    println!("  -o, --output <file>           Write output to a file instead of stdout");
    println!("  -h, --help                    Print this help menu");
}

struct CliArgs {
    words: Option<RangeOrExact>,
    bytes: Option<RangeOrExact>,
    seed: Option<u64>,
    infinite: bool,
    pdf: bool,
    output: Option<String>,
}

fn parse_args() -> CliArgs {
    let mut args = std::env::args().skip(1);
    let mut cli_words = None;
    let mut cli_bytes = None;
    let mut cli_seed = None;
    let mut cli_infinite = false;
    let mut cli_pdf = false;
    let mut cli_output = None;

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
            "-i" | "--infinite" => {
                cli_infinite = true;
            }
            "-p" | "--pdf" => {
                cli_pdf = true;
            }
            "-o" | "--output" => {
                if let Some(val_str) = args.next() {
                    cli_output = Some(val_str);
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
    if cli_infinite && (cli_words.is_some() || cli_bytes.is_some()) {
        eprintln!("Error: --infinite parameter is mutually exclusive with --words and --bytes.");
        std::process::exit(1);
    }
    if cli_pdf && cli_infinite {
        eprintln!("Error: --pdf parameter is mutually exclusive with --infinite.");
        std::process::exit(1);
    }

    CliArgs {
        words: cli_words,
        bytes: cli_bytes,
        seed: cli_seed,
        infinite: cli_infinite,
        pdf: cli_pdf,
        output: cli_output,
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

    // 5. Setup output writer (file or stdout)
    let mut writer: Box<dyn Write> = match &cli_args.output {
        Some(path) => {
            let file = std::fs::File::create(path).unwrap_or_else(|e| {
                eprintln!("Error creating output file {path}: {e}");
                std::process::exit(1);
            });
            Box::new(std::io::BufWriter::new(file))
        }
        None => Box::new(std::io::BufWriter::new(std::io::stdout().lock())),
    };

    // 6. Generate output based on parameters
    let res = if cli_args.infinite {
        generate_infinite(&mut rng, &words, &cdf, sum, &mut writer)
    } else if cli_args.pdf {
        let text_res = if let Some(bytes_range) = cli_args.bytes {
            let mut buf = Vec::new();
            let target_bytes = bytes_range.resolve(&mut rng);
            generate_by_bytes(target_bytes, &mut rng, &words, &cdf, sum, &mut buf)
                .map(|()| buf)
        } else {
            let mut buf = Vec::new();
            let target_words = cli_args
                .words
                .unwrap_or(RangeOrExact::Exact(100))
                .resolve(&mut rng);
            generate_by_words(target_words, &mut rng, &words, &cdf, sum, &mut buf)
                .map(|()| buf)
        };

        match text_res {
            Ok(buf) => {
                let text_str = String::from_utf8(buf).expect("Generated text is not valid UTF-8");
                let pdf_bytes = dutch_gen::generate_pdf(&text_str);
                writer.write_all(&pdf_bytes).and_then(|()| writer.flush())
            }
            Err(e) => Err(e),
        }
    } else if let Some(bytes_range) = cli_args.bytes {
        let target_bytes = bytes_range.resolve(&mut rng);
        generate_by_bytes(target_bytes, &mut rng, &words, &cdf, sum, &mut writer)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
    } else {
        let target_words = cli_args
            .words
            .unwrap_or(RangeOrExact::Exact(100))
            .resolve(&mut rng);
        generate_by_words(target_words, &mut rng, &words, &cdf, sum, &mut writer)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
    };

    if let Err(e) = res {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        } else {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
