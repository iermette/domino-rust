//! Embarque les avatars de `web/avatars/*.png` dans le binaire (include_bytes!).
//! Déposer un PNG dans le dossier puis recompiler suffit — aucun serveur de fichiers externe.

use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let avatars_dir = manifest.join("web/avatars");
    let mut entries: Vec<String> = Vec::new();

    if avatars_dir.is_dir() {
        let mut files: Vec<PathBuf> = fs::read_dir(&avatars_dir)
            .expect("lecture du dossier web/avatars")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                ext == "png"
            })
            .collect();
        files.sort_by_key(|p| {
            p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string()
        });
        for f in &files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            entries.push(format!(
                "    (\"{}\", include_bytes!(r#\"{}\"#)),",
                name,
                f.display()
            ));
        }
    }

    let gen = format!(
        "// Généré par build.rs — ne pas éditer.\npub const AVATARS: &[(&str, &[u8])] = &[\n{}\n];\n",
        entries.join("\n")
    );
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR non défini par Cargo");
    fs::write(PathBuf::from(out_dir).join("avatars_gen.rs"), gen)
        .expect("écriture de avatars_gen.rs");

    println!("cargo:rerun-if-changed=web/avatars");
}
