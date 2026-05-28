# Dutch Random Text Generator

A lightweight, self-contained Rust CLI application to generate pseudo-Dutch text that is statistically representative of real Dutch word frequencies, but does not make grammatical sense. Perfect for generating search engine indexing test datasets.

## Performance
- **Binary Size**: 2.5 MB (fully self-contained, no external database files needed at runtime).
- **Generation Speed**: ~35 MB/s throughput (1 million words in 0.14s).
- **Startup Latency**: < 15ms.

## Usage
```bash
./dutch-gen [options]

Options:
  -w, --words <exact|min-max>   Generate text with exact words or range of words (e.g., 500 or 100-500)
  -b, --bytes <exact|min-max>   Generate text with exact bytes or range of bytes (e.g., 1000 or 500-1500)
  -s, --seed <u64>              Specify seed for reproducible output
  -h, --help                    Print this help menu
```

### Examples
```bash
# Generate exactly 500 words
./dutch-gen -w 500

# Generate between 500 and 1500 bytes reproducibly
./dutch-gen -b 500-1500 -s 42
```

## Data Citation & License

This project embeds the **SUBTLEX-NL** frequency database. Consequently, this project is licensed under the **Creative Commons Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0)** license.

### Attribution
If you distribute this tool or its generated datasets, you must credit the original creators of the SUBTLEX-NL corpus:

> **Keuleers, E., Brysbaert, M., & New, B. (2010).** *SUBTLEX-NL: A new frequency measure for Dutch words based on film subtitles.* Behavior Research Methods, 42, 643-650.
>
> **Van Heuven, W.J.B., Mandera, P., Keuleers, E., & Brysbaert, M. (2014).** *Subtlex-UK: A new and improved word frequency database for British English.* Quarterly Journal of Experimental Psychology, 67, 1176-1190. (for information about Zipf values)
>
> **Brysbaert, M., Mandera, P., & Keuleers, E. (2018).** *The word frequency effect in word processing: An updated review.* Current Directions in Psychological Science, 27, 45-50. (more information about the Zipf measure)
>
> Project website: [https://osf.io/3d8cx](https://osf.io/3d8cx)



