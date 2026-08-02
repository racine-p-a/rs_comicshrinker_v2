use clap::Parser;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use archive::{ArchiveExtractor, ArchiveFormat};
use image::{ImageReader};
use ravif::{Encoder, Img};
use rgb::{RGB8, RGBA8};
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

// todo documenter que nasm est nécessaire à la compilation
// todo résolution maximale -> meilleure compression -> proposer cette option
// todo conserver les métadonnées d'origine ;
// todo paralléliser la conversion AVIF (rayon) ;
// todo accepter plus de formats en entrée/sortie
// todo sortie console en couleurs

#[derive(Parser)]
struct Cli {
    /// The path to the archive to convert.
    path_to_archive: std::path::PathBuf,
    path_to_output: std::path::PathBuf,
}

/// todo doc
/// todo tests
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    // Input acceptable ?
    if !is_input_acceptable(&args.path_to_archive) {
        panic!("Selected output is not acceptable.");
    }

    // Output acceptable ?
    if !is_output_acceptable(&args.path_to_output, &args.path_to_archive) {
        panic!("Selected output is not acceptable.");
    }
    println!("Output destination is acceptable : {:?}", &args.path_to_output);

    // Temporary folder to work with
    let tmp_dir = TempDir::new()?;
    let path = tmp_dir.keep(); // todo à commenter ? car cela garde le fchier temporaire, l'idéal serait de le supprimer

    // Archive extraction
    extract_to_folder(&args.path_to_archive, &path);

    // Image conversion
    convert_directory(&path)?;

    let output = PathBuf::from(&args.path_to_output);

    create_archive(path.as_path(), &output)?;

    Ok(())
}

/// todo doc
/// todo tests
fn is_input_acceptable(path_to_input: &Path)-> bool {
    // Checks
    // - file exists
    // - not a repertory
    // - is a true file
    // - extension acceptable
    // - file is readable
    // - file is not empty

    // Check : file exists
    if !path_to_input.exists() {
        println!("File not found : {}", path_to_input.display());
        return false;
    }

    // Check : not a repertory
    if path_to_input.is_dir() {
        println!("Input must be a file, not a directory.");
        return false;
    }

    // Check : is a true file
    if !path_to_input.is_file() {
        println!("Input is not a regular file.");
        return false;
    }

    // Check : acceptable extension
    if let Some(ext) = path_to_input.extension().and_then(|e| e.to_str()) {
        if !ext.eq_ignore_ascii_case("cbz")
            && !ext.eq_ignore_ascii_case("cbt")
        {
            println!("Input extension is not accepted.");
            return false;
        }
    } else {
        println!("Input file has no extension.");
        return false;
    }

    // Check : file is readable
    if let Err(e) = std::fs::File::open(path_to_input) {
        println!("Cannot read input file: {}", e);
        return false;
    }

    // Check : file is not empty
    let metadata = match std::fs::metadata(path_to_input) {
        Ok(metadata) => metadata,
        Err(error) => {
            println!("Cannot read metadata: {}", error);
            return false;
        }
    };
    if metadata.len() == 0 {
        println!("Input file is empty.");
        return false;
    }

    // All cheks are passed
    true
}

/// todo doc
/// todo tests
fn is_output_acceptable(path_to_output: &Path, path_to_input: &Path)-> bool {
    // Checks :
    // - file already exists
    // - parent folder does not exist
    // - not a directory
    // - acceptable extensions (cbz, zip)
    // - output is different of input

    // Check : file already exists
    if path_to_output.exists() {
        println!("File already exists : {:?}", path_to_output);
        return false;
    }

    // Check : output != input
    let input = match path_to_input.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let output = if path_to_output.is_absolute() {
        path_to_output.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(path_to_output)
    };

    if input == output {
        println!("Input and output must be different.");
        return false;
    }

    // Check : parent folder does not exist
    let parent = path_to_output.parent().unwrap_or(Path::new("."));
    if !parent.as_os_str().is_empty() && !parent.exists() {
        println!("Parent folder for output does not exist : {:?}", parent);
        return false;
    }

    // Check : not a directory
    if path_to_output.is_dir(){
        println!("Output file must not be a repertory : {:?}", path_to_output);
        return false;
    }

    // Check : output extension
    let acceptable_extensions_for_output = ["cbz", "zip"];
    let output_extension = match path_to_output.extension().and_then(|e| e.to_str())
    {
        Some(ext) => ext,
        None => {
            println!("Output file must have an extension : {:?}",path_to_output);
            return false;
        }
    };
    if(!acceptable_extensions_for_output.contains(&output_extension)){
        println!("Output extension not (yet?) accepted : {:?}", output_extension);
        println!("Output extensions accepted today : {:?}", acceptable_extensions_for_output);
        return false;
    }
    true
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
    const MAX_WIDTH: u32 = 1920;
    const MAX_HEIGHT: u32 = 1920;
    let img = ImageReader::open(path)?.decode()?;
    let resized = if img.width() > MAX_WIDTH || img.height() > MAX_HEIGHT {
        println!("Image too big : {}", path.display());
        img.thumbnail(MAX_WIDTH, MAX_HEIGHT)
    } else {
        img
    };
    let rgb = resized.to_rgb8();
    let is_gray = is_grayscale(&rgb);
    println!("is_gray = {}", is_gray);
    if is_gray {
        println!("{} : grayscale", path.display());
    } else {
        println!("{} : color", path.display());
    }

    let pixels: Vec<RGB8> = rgb
        .pixels()
        .map(|p| RGB8::new(p[0], p[1], p[2]))
        .collect();

    let img = Img::new(
        pixels.as_slice(),
        rgb.width() as usize,
        rgb.height() as usize,
    );

    // The conversion differs only by quality : 20 for gray_scale, 30 for colors
    let result;
    if is_gray {
        result = Encoder::new()
            .with_quality(20.0)
            .with_speed(3)
            .encode_rgb(img)?;
    } else {
        result = Encoder::new()
            .with_quality(30.0)
            .with_speed(3)
            .encode_rgb(img)?;
    }

    let output = path.with_extension("avif");
    fs::write(&output, result.avif_file)?;
    if output.exists() {
        fs::remove_file(path)?;
    }

    Ok(())
}

/// todo doc
/// todo tests
fn is_grayscale(img: &image::RgbImage) -> bool {
    let total = img.pixels().count();

    let gray = img.pixels().filter(|p| {
        let r = p[0] as i16;
        let g = p[1] as i16;
        let b = p[2] as i16;

        (r - g).abs() <= 10 &&
            (g - b).abs() <= 10
    }).count();

    let ratio = gray as f64 / total as f64;
    ratio > 0.90
}

/// todo doc
/// todo tests
fn convert_directory(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
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