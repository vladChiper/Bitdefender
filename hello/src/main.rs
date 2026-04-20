use std::fs::File;
use std::io::BufReader;
use zip::ZipArchive;


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
        let (_a, b) = filename.rsplit_once('/').unwrap();
        if b == "" {
            continue;
        }
        println!("Filename: {}", b);
        
    }
    Ok(())
}