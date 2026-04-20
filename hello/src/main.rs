use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

fn display_filenames(paths: Vec<&str>) {
    for path in paths {
        if let Some(filename) = Path::new(path).file_name() {
            if let Some(name) = filename.to_str() {
                println!("{}", name);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Open the file
    let file = File::open("test.zip")?;
    let reader = BufReader::new(file);

    // 2. Initialize the ZIP archive
    let mut archive = ZipArchive::new(reader)?;

    // 3. Iterate through files
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let filename = file.name();
        let (a, b) = filename.rsplit_once('/').unwrap();
        if(b == "") {
            continue;
        }
        println!("Filename: {}", b);
        
        // Example: Reading content of a file
        // use std::io::Read;
        // file.read_to_end(&mut buffer)?;
    }
    Ok(())
}