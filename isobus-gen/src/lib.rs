mod manufacturer;

use std::path::PathBuf;

pub use manufacturer::generate_manufacturer_enum;

pub fn get_iso_export_dir() -> eyre::Result<PathBuf> {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../data/iso-export"));
    eyre::ensure!(dir.exists(), "iso-export/ directory {dir:?} does not exist");
    Ok(dir)
}

fn get_iso_export_csv(name: &str) -> eyre::Result<PathBuf> {
    let dir = get_iso_export_dir()?;
    let path = dir.join(name);
    eyre::ensure!(path.exists(), "CSV file {path:?} does not exist");
    Ok(path)
}

fn get_csv_reader(name: &str) -> eyre::Result<csv::Reader<std::fs::File>> {
    let path = get_iso_export_csv(name)?;
    let file = std::fs::File::open(&path)?;
    let reader = csv::Reader::from_reader(file);
    Ok(reader)
}
