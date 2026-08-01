use clap::Parser;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use archive::{ArchiveExtractor, ArchiveFormat};
use image::{ImageReader};
use ravif::{Encoder, Img};
use rgb::RGBA8;
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

// todo documenter que nasm est nécessaire à la compilation
// todo meilleure compression si noir et blanc -> proposer cette option
// todo résolution maximale -> meilleure compression -> proposer cette option
// todo conserver les métadonnées d'origine ;
// todo paralléliser la conversion AVIF (rayon) ;

#[derive(Parser)]
struct Cli {
    /// The path to the archive to convert.
    path_to_archive: std::path::PathBuf,
}

/// todo doc
/// todo tests
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
    let path = tmp_dir.keep(); // todo à commenter ? car cela garde le fchier temporaire, l'idéal serait de le supprimer

    // Archive extraction
    extract_to_folder(&args.path_to_archive, &path);

    // Image conversion
    convert_directory(&path)?;

    let output = PathBuf::from("result.cbz");

    create_archive(path.as_path(), &output)?;

    Ok(())
}

/// todo doc
/// todo tests
fn create_archive(source_dir: &Path, output_file: &Path,) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output_file)?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(source_dir) {
        let entry = entry?;
        let path = entry.path();

        // On ignore le dossier racine
        if path == source_dir {
            continue;
        }

        let relative_path = path.strip_prefix(source_dir)?;

        if path.is_dir() {
            zip.add_directory(
                relative_path.to_string_lossy(),
                options,
            )?;
        } else {
            zip.start_file(
                relative_path.to_string_lossy(),
                options,
            )?;

            let data = fs::read(path)?;
            zip.write_all(&data)?;
        }
    }

    zip.finish()?;

    Ok(())
}


/// todo doc
/// todo tests
fn convert_to_avif(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // todo more explanation
    let img = ImageReader::open(path)?.decode()?;

    let rgba = img.to_rgba8();


    let width = rgba.width();
    let height = rgba.height();

    let pixels: Vec<RGBA8> = rgba
        .pixels()
        .map(|p| RGBA8::new(p[0], p[1], p[2], p[3]))
        .collect();

    let img = Img::new(
        pixels.as_slice(),
        rgba.width() as usize,
        rgba.height() as usize,
    );

    let result = Encoder::new()
        .with_quality(30.0)
        .with_speed(4)
        .encode_rgba(img)?;

    let output = path.with_extension("avif");

    fs::write(&output, result.avif_file)?;

    if output.exists() {
        fs::remove_file(path)?;
    }

    Ok(())
}

/// todo doc
/// todo tests
fn convert_directory(
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error>> {

    for entry in WalkDir::new(directory) {
        let entry = entry?;

        let path = entry.path();

        if path.is_file() && is_image(path) {
            println!("Conversion : {}", path.display());

            convert_to_avif(path)?;
        }
    }

    Ok(())
}

/// todo doc
/// todo tests
fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some(
            "jpg" |
            "jpeg" |
            "png" |
            "webp"
        )
    )
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
