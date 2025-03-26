# DumpFS: Directory Context Generator for LLMs

`dumpfs` is a command-line tool that generates an XML representation of directory contents, designed specifically for providing context to Large Language Models (LLMs) for coding tasks.

## Features

- Recursively scans directories and generates structured XML output
- Includes file content with CDATA wrapping
- Handles different file types (text, binary, symlinks)
- Provides file metadata (size, modification time, permissions)
- Supports pattern-based inclusion and exclusion of files
- Parallel processing for better performance
- Progress tracking with ETA

## Installation

### From Cargo

```bash
cargo install dumpfs
```

### From Source

```bash
git clone https://github.com/yourusername/dumpfs.git
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
    --threads <N>                               Number of threads to use for processing
```

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
```

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

## License

MIT
