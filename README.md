# PMI - Protect My Images

A fast, privacy-focused CLI tool that strips metadata from images. Built in pure Rust with zero external dependencies.

## Why PMI?

Digital images often contain hidden metadata that can reveal sensitive information:
- **GPS coordinates** - Where the photo was taken
- **Camera details** - Device make, model, serial number
- **Timestamps** - When the photo was taken
- **Software info** - Editing history
- **Personal data** - Author name, copyright, comments

PMI removes all this metadata while preserving your image quality.

## Features

- **Zero dependencies** - Pure Rust, no external crates required
- **Multiple formats** - JPEG, PNG, GIF, WebP, TIFF
- **Batch processing** - Process entire directories
- **Safe by default** - Creates new files (original untouched)
- **In-place mode** - Optionally overwrite originals
- **Dry-run mode** - Preview changes without modifying files
- **Progress bar** - Visual feedback for batch operations
- **Colored output** - Clear success/error/warning messages

## Installation

### From Source

```bash
git clone <repository-url>
cd pmi
cargo build --release
```

The binary will be at `./target/release/pmi`.

### Install to PATH

```bash
cargo install --path .
```

## Usage

### Basic Usage

```bash
# Clean a single image (creates photo_clean.jpg)
pmi photo.jpg

# Clean multiple images
pmi photo1.jpg photo2.png photo3.gif

# Clean all images in current directory
pmi *.jpg *.png
```

### Output Options

```bash
# Overwrite original files (use with caution!)
pmi -i photo.jpg
pmi --in-place photo.jpg

# Save to a specific directory
pmi -o ./cleaned/ photo.jpg
pmi --output-dir ./cleaned/ *.jpg

# Force overwrite existing output files
pmi -f -o ./cleaned/ photo.jpg
```

### Directory Processing

```bash
# Process all supported images in a directory
pmi ./photos/

# Process directories recursively
pmi -r ./photos/
pmi --recursive ./photos/
```

### Preview & Verbose

```bash
# Dry run - see what would be done without making changes
pmi -n photo.jpg
pmi --dry-run ./photos/

# Verbose output - show detailed processing info
pmi -v photo.jpg

# Combine dry-run with verbose for full preview
pmi -n -v ./photos/
```

### Quiet Mode

```bash
# Suppress all output except errors
pmi -q photo.jpg
```

## Supported Formats

| Format | Extensions | Metadata Removed |
|--------|------------|------------------|
| JPEG | `.jpg`, `.jpeg`, `.jpe`, `.jfif` | EXIF, XMP, IPTC, Comments |
| PNG | `.png` | tEXt, zTXt, iTXt, eXIf, tIME chunks |
| GIF | `.gif` | Comment extensions, Application extensions (except NETSCAPE for animations) |
| WebP | `.webp` | EXIF, XMP chunks |
| TIFF | `.tif`, `.tiff` | EXIF IFD, GPS IFD, XMP, IPTC, Make, Model, Software, DateTime, Artist, Copyright |

## Examples

### Single File
```bash
$ pmi vacation.jpg
✓ Cleaned vacation.jpg → vacation_clean.jpg (removed 12.4 KB)

╭──────────────────────────────────────╮
│           Processing Complete        │
├──────────────────────────────────────┤
│  ✓ Processed:  1 files               │
│                                      │
│  Metadata removed: 12.4 KB           │
│  Time elapsed: 0.0s                  │
╰──────────────────────────────────────╯
```

### Batch Processing
```bash
$ pmi -r ./photos/
ℹ Found 156 image file(s) to process
[████████████████████] 156/156 files (100%) - IMG_2024.jpg
✓ Cleaned IMG_0001.jpg → IMG_0001_clean.jpg (removed 8.2 KB)
✓ Cleaned IMG_0002.jpg → IMG_0002_clean.jpg (removed 7.9 KB)
...

╭──────────────────────────────────────╮
│           Processing Complete        │
├──────────────────────────────────────┤
│  ✓ Processed:  156 files             │
│                                      │
│  Metadata removed: 1.2 MB            │
│  Time elapsed: 2.3s                  │
╰──────────────────────────────────────╯
```

### Dry Run
```bash
$ pmi -n -v ./photos/
ℹ Found 3 image file(s) to process
✓ Would clean photo1.jpg (would remove 15.2 KB)
✓ Would clean photo2.png (would remove 2.1 KB)
⚠ Skipped document.pdf: Unsupported format
```

## Command Reference

```
USAGE:
    pmi [OPTIONS] <PATHS>...

ARGUMENTS:
    <PATHS>...    Image files or directories to process

OPTIONS:
    -o, --output-dir <DIR>    Save cleaned images to specified directory
    -r, --recursive           Process directories recursively
    -f, --force               Overwrite existing output files
    -i, --in-place            Modify files in place (default: create *_clean suffix)
    -v, --verbose             Show detailed processing information
    -q, --quiet               Suppress all output except errors
    -n, --dry-run             Show what would be done without making changes
    -h, --help                Print help message
    -V, --version             Print version information
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success - all files processed |
| 1 | Failure - one or more files failed to process |

## How It Works

PMI parses image files at the binary level, identifying and removing metadata segments while preserving the actual image data:

### JPEG
Removes APP1 (EXIF/XMP), APP2-APP12, APP13 (IPTC), APP15, and COM (comment) segments. Preserves APP0 (JFIF), quantization tables, Huffman tables, and image scan data.

### PNG
Filters out ancillary chunks containing metadata (tEXt, zTXt, iTXt, eXIf, tIME) while preserving critical chunks (IHDR, PLTE, IDAT, IEND).

### GIF
Removes comment extensions and application extensions (except NETSCAPE2.0 which controls animation looping). Preserves image data and graphics control extensions.

### WebP
Strips EXIF and XMP chunks from the RIFF container while preserving VP8/VP8L image data, animation frames, and alpha channels.

### TIFF
Filters IFD (Image File Directory) entries, removing metadata tags while preserving essential image structure tags.

## Building from Source

### Requirements
- Rust 2024 edition (rustc 1.85+)

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with arguments
cargo run -- photo.jpg
cargo run -- -v -r ./photos/
```

## Project Structure

```
pmi/
├── Cargo.toml              # Project manifest
├── src/
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library exports
│   ├── cli.rs              # Argument parsing
│   ├── error.rs            # Error types
│   ├── processor.rs        # File processing pipeline
│   ├── terminal/
│   │   ├── mod.rs          # Terminal module
│   │   ├── colors.rs       # ANSI colors & styling
│   │   └── progress.rs     # Progress bar & summary
│   └── formats/
│       ├── mod.rs          # Format detection
│       ├── jpeg.rs         # JPEG metadata stripping
│       ├── png.rs          # PNG metadata stripping
│       ├── gif.rs          # GIF metadata stripping
│       ├── webp.rs         # WebP metadata stripping
│       └── tiff.rs         # TIFF metadata stripping
└── tests/
    └── integration.rs      # Integration tests
```

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please ensure:
1. All tests pass (`cargo test`)
2. Code follows Rust conventions
3. No external dependencies are added
