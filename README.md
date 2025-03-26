# DumpFS: Directory Context Generator for LLMs

`dumpfs` is a command-line tool that generates an XML representation of directory contents, designed specifically for providing context to Large Language Models (LLMs) for coding tasks.

## Features

- Recursively scans directories and generates structured XML output
- Includes file content with CDATA wrapping
- Handles different file types (text, binary, symlinks)
- Provides file metadata (size, modification time, permissions)
- Supports pattern-based inclusion and exclusion of files
- Respects `.gitignore` files for intelligent filtering
- Parallel processing for better performance
- Progress tracking with ETA and detailed file statistics
- Beautiful Unicode progress display with real-time file information
- Comprehensive summary of scanned content with LLM token estimation

## Installation

### From Source

```bash
git clone https://github.com/kkharji/dumpfs.git
cd dumpfs
cargo build --release
```

The binary will be available at `target/release/dumpfs`.

## Usage

```
dumpfs [DIRECTORY_PATH] [OUTPUT_FILE] [OPTIONS]

OPTIONS:
    --ignore-patterns <pattern1,pattern2,...>    Comma-separated list of patterns to ignore
    --include-patterns <pattern1,pattern2,...>   Comma-separated list of patterns to include
    --threads <N>                                Number of threads to use for processing
    --respect-gitignore <BOOL>                   Whether to respect .gitignore files (default: true)
    --gitignore-path <PATH>                      Path to custom .gitignore file
```

When running the command, you'll see a beautiful progress display showing:

- Real-time progress with an animated spinner
- Current file being processed
- Progress bar showing completion percentage
- Processing speed (files per second)
- Estimated time remaining

After completion, you'll get a comprehensive summary showing file statistics and token estimation for LLM usage.

### Examples

```bash
# Process current directory
dumpfs

# Process specific directory with custom output file
dumpfs /path/to/project project_context.xml

# Ignore specific patterns
dumpfs --ignore-patterns "*.log,*.tmp,*.bak"

# Include only specific patterns
dumpfs --include-patterns "*.rs,*.toml,*.md"

# Use 8 threads for processing
dumpfs --threads 8

# Disable .gitignore respect
dumpfs --respect-gitignore false

# Use custom gitignore file
dumpfs --gitignore-path /path/to/custom/gitignore
```

## GitIgnore Support

By default, `dumpfs` respects `.gitignore` files in the project directory. This means that files and directories that would be ignored by Git are also ignored by `dumpfs`. This is useful for excluding build artifacts, dependencies, and other files that are not relevant to the codebase.

You can disable this behavior with the `--respect-gitignore false` option, or specify a custom gitignore file with the `--gitignore-path` option.

## Output Format

The tool generates an XML file with the following structure:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<directory_scan timestamp="2025-03-26T12:34:56+00:00">
  <system_info>
    <hostname>your-hostname</hostname>
    <os>linux</os>
    <kernel>unix</kernel>
  </system_info>
  <directory name="project" path="project">
    <metadata>
      <size>4096</size>
      <modified>2025-03-26T12:34:56+00:00</modified>
      <permissions>755</permissions>
    </metadata>
    <contents>
      <file name="example.rs" path="project/example.rs">
        <metadata>
          <size>1024</size>
          <modified>2025-03-26T12:34:56+00:00</modified>
          <permissions>644</permissions>
        </metadata>
        <content><![CDATA[fn main() {
    println!("Hello, world!");
}]]></content>
      </file>
      <!-- More files and directories -->
    </contents>
  </directory>
</directory_scan>
```

## Example Output

When running `dumpfs`, you'll initially see scanning messages and a progress bar. After completion, the progress information is automatically cleared, and you'll see a comprehensive summary with the processed files followed by extraction statistics:

```
📋  PROCESSED FILES
╭────────────────┬───────┬─────────────╮
│ File Path      │ Lines │ Est. Tokens │
├────────────────┼───────┼─────────────┤
│ src/scanner.rs │ 461   │ 4.1K        │
│ src/tests.rs   │ 330   │ 2.9K        │
│ src/report.rs  │ 272   │ 2.1K        │
│ src/writer.rs  │ 202   │ 2.0K        │
│ README.md      │ 170   │ 1.5K        │
│ src/utils.rs   │ 209   │ 1.2K        │
│ src/config.rs  │ 119   │ 928         │
│ src/main.rs    │ 113   │ 870         │
│ src/types.rs   │ 95    │ 538         │
│ src/lib.rs     │ 27    │ 188         │
│ Cargo.toml     │ 29    │ 135         │
╰────────────────┴───────┴─────────────╯

✅  EXTRACTION COMPLETE
╭─────────────────────────┬─────────────────────╮
│ Metric                  │ Value               │
├─────────────────────────┼─────────────────────┤
│ 📂 Output File          │ .dumpfs.context.xml │
│ ⏱️ Process Time          │ 7.647ms             │
│ 📄 Files Processed      │ 12                  │
│ 📝 Total Lines          │ 2.1K                │
│ 📦 Estimated LLM Tokens │ 16.7K tokens        │
╰─────────────────────────┴─────────────────────╯
```

The output provides:
- A detailed breakdown of each file with line counts and token estimates
- File paths displayed relative to the project root
- Human-readable numbers with K suffixes for large values
- Total processing time with millisecond precision
- Total number of files processed
- Total line count
- Estimated token usage for LLM context

This information is particularly valuable when preparing context for LLMs, as it helps you understand the size and composition of the context you're providing.

## License

MIT
