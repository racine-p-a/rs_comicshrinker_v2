use clap::Parser;
use std::io;
use std::path::Path;

#[derive(Parser)]
struct Cli {
    /// The path to the archive to convert.
    path_to_archive: std::path::PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    // The file exists ?
    check_archive_exists(&args.path_to_archive)?;
    println!("file ok");

    // The file has a proper extension ?
    // todo


    Ok(())
}

fn check_archive_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Archive introuvable : {}", path.display()),
        ))
    }
}
