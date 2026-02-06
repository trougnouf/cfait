//! Small utility to inspect UniFFI metadata and components embedded in a compiled cdylib.
//!
//! Usage:
//!   cargo run --bin inspect_bindgen -- <path-to-lib>
//! or (if run standalone after adding to a workspace binary):
//!   cargo run --manifest-path ./cfait/Cargo.toml -p inspect_bindgen -- target/release/libcfait.so
//!
//! Defaults to `target/release/libcfait.so` if no argument is provided.
//
//! This file is intentionally small and defensive: it reads the library bytes, asks
//! UniFFI's macro metadata extractor to parse the embedded metadata, groups the
//! metadata into crate-level groups, and prints a short summary to stdout. The goal
//! is to answer the question: "what metadata/components does the binder actually
//! see inside the compiled library?"
use anyhow::{Context, Result};
use camino::Utf8Path;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Pick the library file path from the first argument, falling back to the
    // common default used in this project.
    let lib_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/release/libcfait.so"));

    println!("Inspecting UniFFI metadata in: {}", lib_path.display());

    // Convert path to a UTF-8 path (required by the bindgen loader API).
    let lib_str = lib_path
        .to_str()
        .with_context(|| format!("Library path is not valid UTF-8: {}", lib_path.display()))?;
    let lib_utf8 = Utf8Path::new(lib_str);

    // First: read the library bytes directly and attempt to extract UniFFI macro metadata.
    // This shows what was embedded by the proc-macros, independent of the loader's config.
    let lib_bytes = std::fs::read(&lib_path)
        .with_context(|| format!("Failed to read library bytes from {}", lib_path.display()))?;

    match uniffi_bindgen::macro_metadata::extract_from_bytes(&lib_bytes) {
        Ok(items) => {
            println!(
                "Macro metadata extractor found {} item(s) in the library.",
                items.len()
            );
            // Also show which crates (namespaces) are present per the raw metadata grouping.
            let mut md_groups = uniffi_meta::create_metadata_groups(&items);
            if let Err(e) = uniffi_meta::group_metadata(&mut md_groups, items.clone()) {
                eprintln!("Warning: grouping raw metadata returned an error: {:#}", e);
            }
            println!(
                "Raw metadata groups discovered (crates): {:?}",
                md_groups.keys().collect::<Vec<_>>()
            );
        }
        Err(e) => {
            eprintln!("Macro metadata extractor failed: {:#}", e);
        }
    }

    // Now use the public BindgenLoader API to load metadata and components (this may
    // augment/merge UDL-derived metadata via bindgen paths / config).
    let bindgen_paths = uniffi_bindgen::BindgenPaths::default();
    let loader = uniffi_bindgen::BindgenLoader::new(bindgen_paths);

    let metadata = loader
        .load_metadata(lib_utf8)
        .with_context(|| format!("Failed to load metadata from {}", lib_path.display()))?;
    println!(
        "Loader discovered metadata for crates: {:?}",
        metadata.keys().collect::<Vec<_>>()
    );

    // Convert metadata groups into ComponentInterface objects the bindgen will use.
    let cis = loader
        .load_cis(metadata)
        .with_context(|| "Failed to convert metadata into ComponentInterface instances")?;
    println!("Found {} ComponentInterface(s)", cis.len());

    for ci in &cis {
        // Print basic info about each component. Use public accessors.
        println!(
            "ci crate_name='{}' namespace='{}'",
            ci.crate_name(),
            ci.namespace()
        );
    }

    println!();
    println!("Done.");

    Ok(())
}
