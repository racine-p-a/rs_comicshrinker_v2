use clap::Parser;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use archive::{ArchiveExtractor, ArchiveFormat, ArchiveError};
use tempfile::TempDir;

#[derive(Parser)]
struct Cli {
    /// The path to the archive to convert.
    path_to_archive: std::path::PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    // Path to file exists ?
    match check_archive_exists(&args.path_to_archive) {
        Ok(_) => { println!(); }
        Err(error) => { panic!("{}", error); }
    }

    // File extension acceptable ?
    if !is_file_acceptable(&args.path_to_archive) {
        panic!("File extension is not acceptable.");
    }

    // Temporary folder to work with
    let tmp_dir = TempDir::new()?;
    let path = tmp_dir.keep(); // todo àsupprimer ? car cela garde le fchier temporaire, l'idéal serait de le supprimer

    // Archive extraction
    extract_to_folder(&args.path_to_archive, &path);

    Ok(())
}


/// Extracts an archive (param 1) to folder (param2)
/// TODO tests
fn extract_to_folder(path: &Path, tmp_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let data = fs::read(path)?;
    let extractor = ArchiveExtractor::new();
    let files = extractor.extract(&data, ArchiveFormat::Zip)?;

    for file in files {
        let output_path = tmp_dir.join(&file.path);

        if file.is_directory {
            fs::create_dir_all(&output_path)?;
        } else {
            // Creates parent directory if not already created.
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, &file.data)?;

            //println!("File: {} ({} bytes)", output_path.display(), file.path);
        }
    }
    Ok(())
}

/// todo doc
/// todo test
fn is_file_acceptable(path: &Path) -> bool {
    let acceptable_extensions = ["cbz", "cbt"];
    let extension = path.extension().unwrap().to_os_string().into_string().unwrap();

    if acceptable_extensions.contains(&&*extension) {
        true
    } else{
        false
    }
}


/// todo doc
/// todo test
fn check_archive_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found : {}", path.display()),
        ))
    }
}
