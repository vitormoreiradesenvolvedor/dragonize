//! Lists the embedded images (assets/brand, assets/vendor/mice, assets/vendor/kbd) for src/assets.rs.
use std::fmt::Write;

fn main() {
    let mut out = String::from("pub const FILES: &[(&str, &[u8])] = &[\n");
    for (dir, prefix) in [("assets/brand", ""), ("assets/vendor/mice", "mice/"), ("assets/vendor/kbd", "kbd/")] {
        println!("cargo:rerun-if-changed={dir}");
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        names.retain(|n| n.ends_with(".png") && n != "icon.png");
        names.sort();
        for n in names {
            let key = format!("{prefix}{}", n.trim_end_matches(".png"));
            let _ = writeln!(out, "    ({key:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{dir}/{n}\"))),");
        }
    }
    out.push_str("    (\"vortron\", include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/vortron.png\"))),\n];\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("assets_list.rs"), out).unwrap();
}
