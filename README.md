# Extract-XISO (Rust Edition) 🦀

[![Rust](https://img.shields.io/badge/rust-v1.70%2B-blue.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

A blazing-fast, modern, and multi-threaded rewrite of the classic `extract-xiso` tool in Rust. 

This tool is designed to extract original Xbox and Xbox 360 ISO (XISO) images. By leveraging Rust's safety and modern async/multithreading paradigms, this version drastically outperforms the original C implementation on modern storage devices (SSDs/NVMe).

## ✨ Features

- 🚀 **Blazing Fast**: Uses zero-copy memory mapping (`memmap2`) instead of traditional `lseek`/`read` syscalls.
- ⚡ **Multi-threaded Extraction**: Extracts multiple files simultaneously using `rayon` to saturate your drive's write speeds.
- 🛡️ **Secure by Design**: Built-in protection against Path Traversal attacks (e.g., malicious `../` filenames in the ISO).
- 🧹 **Smart Filtering**: Optionally skip the bulky and usually unnecessary `$SystemUpdate` folders.
- 📊 **Beautiful UI**: Features a live progress bar with ETA and transfer speeds (`indicatif`).
- 📁 **List Mode**: Instantly list the contents of an ISO without extracting it.

## 🛠️ Installation

You will need the [Rust toolchain](https://rustup.rs/) installed on your system.

1. Clone the repository:
   ```bash
   git clone https://github.com/18woldemar/extract-xiso-rs.git
   cd extract-xiso-rs
   ```

2. Build the project in release mode for maximum performance:
   ```bash
   cargo build --release
   ```

3. The compiled binary will be located at `target/release/extract-xiso` (or `extract-xiso.exe` on Windows). You can move it to your system's `PATH` for global access.

## 🚀 Usage

```bash
extract-xiso [OPTIONS] <INPUT>
```

### Examples

**1. Basic Extraction**
Extracts the ISO into a folder named after the ISO file in the current directory.
```bash
extract-xiso game.iso
```

**2. List Files Only**
Instantly print the directory tree and file sizes without extracting anything.
```bash
extract-xiso game.iso -l
```

**3. Skip `$SystemUpdate` Folder**
Extract the game but ignore the Xbox system update partition to save space.
```bash
extract-xiso game.iso -s
```

**4. Specify an Output Directory**
```bash
extract-xiso game.iso -d /path/to/custom/folder
```

**5. Verbose Output**
Extract files and print the name of every file as it is being processed.
```bash
extract-xiso game.iso -v
```

## 📖 Command-Line Options

| Option | Long Option | Description |
| :--- | :--- | :--- |
| `-d` | `--output <DIR>` | Output directory (default: filename without `.iso`) |
| `-l` | `--list` | List files in the XISO (do not extract) |
| `-s` | `--skip-sysupdate` | Skip the `$SystemUpdate` folder |
| `-v` | `--verbose` | Print names of extracted files (verbose mode) |
| `-h` | `--help` | Print help information |
| `-V` | `--version` | Print version information |

## 🧠 Why rewrite in Rust?

The original `extract-xiso` written in C is a legendary tool, but it was designed in an era of spinning hard drives (HDDs). It processes files sequentially and relies heavily on OS-level file seeking (`lseek`). 

This Rust rewrite takes a modern approach:
1. The entire ISO is virtually mapped into memory.
2. The binary tree of the XISO file system is parsed recursively to build a flat list of files.
3. A thread pool spins up, grabbing chunks of memory and writing them directly to disk in parallel. 

The result is an extraction process that is bound only by the sequential read speed of your source drive and the parallel write speed of your destination drive.
