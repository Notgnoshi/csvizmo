use std::io::Write;

fn main() {
    let dir = isobus_gen::get_iso_export_dir().unwrap();
    println!("cargo:rerun-if-changed={dir:?}");

    let code = isobus_gen::generate_manufacturer_enum().unwrap();

    let out = std::env::var("OUT_DIR").unwrap();
    let out = std::path::Path::new(&out).join("manufacturer.rs");
    let mut f = std::fs::File::create(&out).unwrap();
    f.write_all(code.as_bytes()).unwrap();
}
