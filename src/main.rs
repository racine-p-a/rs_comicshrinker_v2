use clap::Parser;
use std::fs::{self, File};
use std::io::{Write};
use std::path::{Path, PathBuf};
use archive::{ArchiveExtractor, ArchiveFormat};
use image::{ImageReader};
use ravif::{BitDepth, Encoder, Img};
use rgb::{RGB8};
use tempfile::TempDir;
use walkdir::WalkDir;
use weaver_unrar::{ExtractOptions, RarArchive};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

// todo documenter que nasm est nécessaire à la compilation
// todo conserver les métadonnées d'origine ;
// todo paralléliser la conversion AVIF (rayon) ;
// todo accepter plus de formats en entrée/sortie
// todo sortie console en couleurs
// todo utiliser une vraie détection de la compression du fichier https://docs.rs/rars/latest/rars/ -> pub use detect::detect_archive_family;


#[derive(Parser)]
struct Cli {
    /// The path to the archive to convert.
    path_to_archive: std::path::PathBuf,
    /// The path to the new archive.
    path_to_output: std::path::PathBuf,
    /// Maximum width/height of the converted image : --maxsize=1920 or --maxsize 1920
    #[arg(long, default_value_t = 1920)]
    max_size: u32,
    /// Keep directory structure in the output archive.
    #[arg(long)]
    no_flatten: bool,
    /// Keep original filenames.
    #[arg(long)]
    no_rename: bool,
}

/// todo doc
/// todo tests
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let flatten = !args.no_flatten;
    let rename = !args.no_rename;

    // Input acceptable ?
    if !is_input_acceptable(&args.path_to_archive) {
        panic!("Selected input is not acceptable.");
    }

    // Output acceptable ?
    if !is_output_acceptable(&args.path_to_output, &args.path_to_archive) {
        panic!("Selected output is not acceptable.");
    }
    println!("Output destination is acceptable : {:?}", &args.path_to_output);

    if flatten {
        println!("File structure will be flattened. If you do not want it, add the option --no-flatten to your command.");
    } else {
        println!("File structure will be kept as is.");
    }

    if rename {
        println!("The image filenames will be renamed. If you do not want it, add the option --no-rename to your command.");
    } else {
        println!("The image filenames will be kept as is.");
    }

    // Temporary folder to work with
    let tmp_dir = TempDir::new()?;

    // Archive extraction
    extract_to_folder(&args.path_to_archive, &tmp_dir.path().to_path_buf())?;

    // Image conversion
    println!("All images will be resized to maximum dimensions of : {} pixels (size ratio will be kept). Use the option \"--max-size=1920\" if you want another maximum.", args.max_size);
    println!("Use the option \"--max-size=1920\" if you want another maximum.");
    println!("Use the option \"--max-size=0\" if you want deactivate this behavior.");
    convert_directory(&tmp_dir.path(), &args.max_size)?;

    let output = PathBuf::from(&args.path_to_output);

    create_archive(tmp_dir.path(), &output, flatten, rename)?;

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
            && !ext.eq_ignore_ascii_case("zip")
            && !ext.eq_ignore_ascii_case("cbr")
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
    if !acceptable_extensions_for_output.contains(&output_extension){
        println!("Output extension not (yet?) accepted : {:?}", output_extension);
        println!("Output extensions accepted today : {:?}", acceptable_extensions_for_output);
        return false;
    }
    true
}

/// todo doc
/// todo tests
fn create_archive(source_dir: &Path, output_file: &Path,to_flatten: bool, to_rename: bool,) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output_file)?;
    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    for entry in WalkDir::new(source_dir) {
        let entry = entry?;
        if entry.path().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    // To get the size of names. 01.avif or 001.avif, etc...
    let digits = files.len().to_string().len();

    for (index, path) in files.iter().enumerate() {
        let archive_path = if to_rename {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin");
            format!("{:0width$}.{}",index + 1,ext,width = digits,)
        } else if to_flatten {
            path.file_name()
                .ok_or("Nom de fichier invalide")?
                .to_string_lossy()
                .into_owned()
        } else {
            path.strip_prefix(source_dir)?
                .to_string_lossy()
                .into_owned()
        };

        zip.start_file(&archive_path, options)?;

        let data = fs::read(path)?;
        zip.write_all(&data)?;
    }

    zip.finish()?;

    Ok(())
}


/// todo doc
/// todo tests
fn convert_to_avif(path: &Path, max_size : &u32) -> Result<(), Box<dyn std::error::Error>> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let resized = if *max_size!=0 && (img.width() > *max_size || img.height() > *max_size) {
        println!("Image too big : {}", path.display());
        img.thumbnail(*max_size, *max_size)
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
    let quality = if is_gray { 20.0 } else { 30.0 };
    let result = Encoder::new()
        .with_quality(quality)
        .with_speed(3)
        .with_bit_depth(BitDepth::Eight) // Eight for better compatibility with reading apps. Better compression with Ten.
        .encode_rgb(img)?;

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
fn convert_directory(directory: &Path, max_size : &u32) -> Result<(), Box<dyn std::error::Error>> {
    for entry in WalkDir::new(directory) {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_image(path) {
            println!("Conversion : {}", path.display());
            convert_to_avif(path, &max_size)?;
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
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "cbz"|"zip" => {
            let data = fs::read(path)?;
            let extractor = ArchiveExtractor::new();
            let files = extractor.extract(&data, ArchiveFormat::Zip)?;

            for file in files {
                let output_path = tmp_dir.join(&file.path);
                if file.is_directory {
                    fs::create_dir_all(&output_path)?;
                } else {
                    if let Some(parent) = output_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&output_path, &file.data)?;
                }
            }
        }
        "cbr" => {
            let file = File::open(path)?;
            let mut archive = RarArchive::open(file)?;
            let options = ExtractOptions::default();
            for index in 0..archive.member_names().len() {
                let member = archive
                    .member_info(index)
                    .ok_or("Invalid RAR member index")?;
                let member_path = Path::new(&member.name);
                let output_path = tmp_dir.join(member_path);
                if member.is_directory {
                    fs::create_dir_all(&output_path)?;
                    continue;
                }
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                archive.extract_member_to_file(
                    index,
                    &options,
                    None,
                    &output_path,
                )?;
            }
        }

        _ => {
            return Err(format!(
                "Unsupported archive format: {}",
                extension
            ).into());
        }
    }
    Ok(())

/*
    let data = fs::read(path)?;
    let extractor = ArchiveExtractor::new();

    let format = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("cbz") => ArchiveFormat::Zip,
        Some("cbr") => ArchiveFormat::Rar,
        _ => return Err("Unsupported archive format".into()),
    };

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

 */
}