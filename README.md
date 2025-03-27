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
- Intelligent caching of tokenized files for faster processing

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
    --model <MODEL>                              LLM model to use for tokenization
```

### Supported Models

The `--model` option enables accurate token counting and caching. Supported models include:

**OpenAI Models:**
- `gpt-4` - GPT-4 (8K context window)
- `gpt-4-turbo` - GPT-4 Turbo (128K context window)
- `gpt4o` - GPT-4o (8K context window)

**Anthropic Models:**
- `sonnet-3.5` - Claude 3.5 Sonnet (200K context window)
- `sonnet-3.7` - Claude 3.7 Sonnet (200K context window)

**HuggingFace Models:**
- `llama-2-7b` - Llama 2 7B (4K context window)
- `llama-3-8b` - Llama 3 8B (8K context window)
- `mistral-small` - Mistral Small (32K context window)
- `mistral-small-24b` - Mistral Small 24B (128K context window)
- `mistral-large` - Mistral Large (128K context window)
- `pixtral-12b` - Pixtral 12B (128K context window)

When a model is specified, `dumpfs` provides exact token counts instead of estimates and caches results for faster processing on subsequent runs.

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

# Use specific model for token counting with caching
dumpfs --model gpt4o
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
╭────────────────────┬─────────────────────────────╮
│ Metric             │ Value                       │
├────────────────────┼─────────────────────────────┤
│ 📂 Output File     │ .dumpfs.context.xml         │
│ ⏱️ Process Time    │ 10.8125ms                   │
│ 📄 Files Processed │ 12                          │
│ 📝 Total Lines     │ 3.0K                        │
│ 📦 LLM Tokens      │ 21.2K tokens (counted)      │
│ 🔄 Cache Hit Rate  │ 100.0% (12 hits / 12 total) │
╰────────────────────┴─────────────────────────────╯
```

The output provides:
- A detailed breakdown of each file with line counts and token counts
- File paths displayed relative to the project root
- Human-readable numbers with K suffixes for large values
- Total processing time with millisecond precision
- Total number of files processed
- Total line count
- Exact token usage for LLM context (when using a model)
- Cache hit rate showing tokenization efficiency

This information is particularly valuable when preparing context for LLMs, as it helps you understand the size and composition of the context you're providing.

## Token Caching

When using the `--model` option, dumpfs implements intelligent caching of tokenized content:

- Only tokenizes files that haven't been processed before or have changed
- Persists cache between runs in `~/.cache/dumpfs/[project_path].token_cache.json`
- Automatically cleans up old cache entries (older than 7 days)
- Reports cache hit rate in the output summary

This caching mechanism significantly improves performance when running the tool multiple times on the same codebase, especially with API-based tokenizers like those from OpenAI or Anthropic.

**First run with caching:**
```
📦 LLM Tokens      │ 21.2K tokens (counted)      │
🔄 Cache Hit Rate  │ 0.0% (0 hits / 12 total)    │
```

**Subsequent runs:**
```
📦 LLM Tokens      │ 21.2K tokens (counted)      │
🔄 Cache Hit Rate  │ 100.0% (12 hits / 12 total) │
```

Tokenization is often the most time-consuming part of the process, especially when using remote API-based tokenizers, so this caching mechanism can dramatically improve performance for repeated scans.

## License

MIT
