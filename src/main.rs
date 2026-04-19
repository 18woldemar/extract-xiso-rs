use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use memmap2::MmapOptions;
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

const XISO_SECTOR_SIZE: usize = 2048;
const XISO_HEADER_OFFSET: usize = 0x10000;
const XISO_HEADER_DATA: &[u8] = b"MICROSOFT*XBOX*MEDIA";

const GLOBAL_LSEEK_OFFSET: u64 = 0x0FD90000;
const XGD3_LSEEK_OFFSET: u64 = 0x02080000;
const XGD1_LSEEK_OFFSET: u64 = 0x18300000;
const OFFSETS: [u64; 4] = [0, GLOBAL_LSEEK_OFFSET, XGD3_LSEEK_OFFSET, XGD1_LSEEK_OFFSET];

const XISO_ATTRIBUTE_DIR: u8 = 0x10;

#[derive(Parser, Debug)]
#[command(
    name = "extract-xiso",
    author,
    version,
    about = "A tool to extract original Xbox and Xbox 360 ISO (XISO) images",
    long_about = None
)]
struct Args {
    /// Path to the .iso file
    #[arg(required = true)]
    input: String,

    /// Output directory (default: filename without .iso)
    #[arg(short = 'd', long)]
    output: Option<String>,

    /// List files in xiso (do not extract)
    #[arg(short = 'l', long)]
    list: bool,

    /// Skip $SystemUpdate folder
    #[arg(short = 's', long)]
    skip_sysupdate: bool,

    /// Print names of extracted files (verbose mode)
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    start_sector: u32,
    size: u32,
}

fn main() {
    let args = Args::parse();
    let input_path = Path::new(&args.input);

    if !input_path.exists() {
        eprintln!("Error: File '{}' not found.", args.input);
        std::process::exit(1);
    }

    // Determine extraction directory
    let out_dir = args.output.unwrap_or_else(|| {
        input_path
            .file_stem()
            .unwrap_or_else(|| std::ffi::OsStr::new("extracted"))
            .to_string_lossy()
            .to_string()
    });
    let out_path = Path::new(&out_dir);

    // Open file and create Memory Map for zero-copy reading
    let file = File::open(input_path).expect("Failed to open ISO file");
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .expect("Failed to map ISO into memory")
    };

    // 1. Search for valid XISO offset
    let mut base_offset: usize = 0;
    let mut valid_iso = false;

    for &offset in &OFFSETS {
        let check_pos = XISO_HEADER_OFFSET + offset as usize;
        if check_pos + 20 <= mmap.len() && &mmap[check_pos..check_pos + 20] == XISO_HEADER_DATA {
            base_offset = offset as usize;
            valid_iso = true;
            break;
        }
    }

    if !valid_iso {
        eprintln!("Error: '{}' is not a valid Xbox ISO image.", args.input);
        std::process::exit(1);
    }

    let header_start = XISO_HEADER_OFFSET + base_offset;
    let root_dir_sector = u32::from_le_bytes(
        mmap[header_start + 20..header_start + 24]
            .try_into()
            .unwrap(),
    );
    let root_dir_size = u32::from_le_bytes(
        mmap[header_start + 24..header_start + 28]
            .try_into()
            .unwrap(),
    );

    if root_dir_sector == 0 || root_dir_size == 0 {
        println!("Image is empty. Operation finished.");
        return;
    }

    if !args.list {
        println!("Analyzing file structure...");
    }

    let root_dir_start = (root_dir_sector as usize * XISO_SECTOR_SIZE) + base_offset;

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    // If we are just listing, we don't want the output directory prefix in the paths
    let base_parse_path = if args.list {
        PathBuf::new()
    } else {
        out_path.to_path_buf()
    };

    parse_dir_node(
        &mmap,
        root_dir_start,
        0,
        &base_parse_path,
        &mut files,
        &mut dirs,
        base_offset,
        args.skip_sysupdate,
    );

    let total_files = files.len();
    let total_bytes: u64 = files.iter().map(|f| f.size as u64).sum();

    // 2. Handle List mode
    if args.list {
        println!("Listing files in {}:\n", input_path.display());
        for f in &files {
            // Unify path separators for cleaner output on all OS
            let display_path = f.path.to_string_lossy().replace("\\", "/");
            println!("{} ({} bytes)", display_path, f.size);
        }
        println!(
            "\nTotal: {} files in {} folders ({:.2} MB)",
            total_files,
            dirs.len(),
            total_bytes as f64 / 1024.0 / 1024.0
        );
        return;
    }

    // 3. Handle Extraction mode
    println!(
        "Found: {} files, {} folders ({:.2} MB)",
        total_files,
        dirs.len(),
        total_bytes as f64 / 1024.0 / 1024.0
    );
    println!("Output directory: {}", out_path.display());

    fs::create_dir_all(out_path).expect("Failed to create output directory");
    for dir in &dirs {
        fs::create_dir_all(dir).expect("Failed to create subdirectory");
    }

    let pb = ProgressBar::new(total_bytes);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    files.par_iter().for_each(|f| {
        if args.verbose {
            pb.println(format!("Extracting: {}", f.path.display()));
        }

        let mut out = File::create(&f.path).expect("Failed to create file");

        let start = (f.start_sector as usize * XISO_SECTOR_SIZE) + base_offset;
        let end = start + f.size as usize;

        let end = std::cmp::min(end, mmap.len());

        if start < mmap.len() {
            let data = &mmap[start..end];
            out.write_all(data).expect("Failed to write to file");
        }

        pb.inc(f.size as u64);
    });

    pb.finish_with_message("Extraction completed successfully!");
}

/// Recursive parser for the directory tree (binary tree)
fn parse_dir_node(
    mmap: &[u8],
    dir_start: usize,
    mut node_offset: usize,
    current_path: &Path,
    files: &mut Vec<FileEntry>,
    dirs: &mut Vec<PathBuf>,
    base_offset: usize,
    skip_sysupdate: bool,
) {
    loop {
        let abs_offset = dir_start + node_offset;

        if abs_offset + 14 > mmap.len() {
            break;
        }

        let tmp = u16::from_le_bytes(mmap[abs_offset..abs_offset + 2].try_into().unwrap());

        // In XISO, 0xFFFF means end of sector data, align offset to the next sector
        if tmp == 0xFFFF {
            if node_offset == 0 {
                return;
            } // Empty directory
            node_offset = (node_offset + (XISO_SECTOR_SIZE - 1)) & !(XISO_SECTOR_SIZE - 1);
            continue;
        }

        let l_offset = tmp;
        let r_offset = u16::from_le_bytes(mmap[abs_offset + 2..abs_offset + 4].try_into().unwrap());
        let start_sector =
            u32::from_le_bytes(mmap[abs_offset + 4..abs_offset + 8].try_into().unwrap());
        let file_size =
            u32::from_le_bytes(mmap[abs_offset + 8..abs_offset + 12].try_into().unwrap());
        let attributes = mmap[abs_offset + 12];
        let name_len = mmap[abs_offset + 13] as usize;

        if abs_offset + 14 + name_len > mmap.len() {
            break;
        }

        let name_bytes = &mmap[abs_offset + 14..abs_offset + 14 + name_len];
        let name = String::from_utf8_lossy(name_bytes).to_string();

        // Process left branch
        if l_offset > 0 {
            parse_dir_node(
                mmap,
                dir_start,
                (l_offset as usize) * 4,
                current_path,
                files,
                dirs,
                base_offset,
                skip_sysupdate,
            );
        }

        // Basic protection against Path Traversal
        if name != "." && name != ".." && !name.contains('/') && !name.contains('\\') {
            // Apply $SystemUpdate skip logic
            let is_sysupdate = name.eq_ignore_ascii_case("$SystemUpdate");

            if !(skip_sysupdate && is_sysupdate) {
                let mut next_path = current_path.to_path_buf();
                next_path.push(&name);

                if (attributes & XISO_ATTRIBUTE_DIR) != 0 {
                    dirs.push(next_path.clone());
                    if file_size > 0 {
                        let sub_dir_start =
                            (start_sector as usize * XISO_SECTOR_SIZE) + base_offset;
                        parse_dir_node(
                            mmap,
                            sub_dir_start,
                            0,
                            &next_path,
                            files,
                            dirs,
                            base_offset,
                            skip_sysupdate,
                        );
                    }
                } else {
                    files.push(FileEntry {
                        path: next_path,
                        start_sector,
                        size: file_size,
                    });
                }
            }
        }

        // Process right branch (Tail recursion replaced with a loop)
        if r_offset > 0 {
            node_offset = (r_offset as usize) * 4;
        } else {
            break;
        }
    }
}
