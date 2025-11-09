use crate::{error::Result, query_pack::QueryPack, session::Session};
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub enum PackFormat {
    Yaml,
    Json,
}

pub fn execute(session_name: String, output: Option<PathBuf>, format: PackFormat) -> Result<()> {
    // Load session
    eprintln!("Loading session '{}'...", session_name);
    let session = Session::load(&session_name)?;

    // Convert to query pack
    eprintln!("Converting session to query pack...");
    let pack = session.to_query_pack()?;

    // Validate generated pack
    pack.validate()?;

    // Determine output path
    let output_path = if let Some(path) = output {
        path
    } else {
        // Default: ~/.kql-panopticon/packs/<session-name>.yaml
        let extension = match format {
            PackFormat::Yaml => "yaml",
            PackFormat::Json => "json",
        };

        let pack_name = session
            .name
            .rsplit_once('_')
            .and_then(|(prefix, suffix)| {
                if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() >= 6 {
                    Some(prefix)
                } else {
                    None
                }
            })
            .unwrap_or(&session.name);

        QueryPack::get_library_path(&format!("{}.{}", pack_name, extension))?
    };

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Save pack
    eprintln!("Saving query pack...");
    pack.save_to_file(&output_path)?;

    eprintln!("✓ Successfully exported session to query pack");
    eprintln!("  Pack name: {}", pack.name);
    eprintln!("  Queries: {}", pack.get_queries().len());
    eprintln!("  Output: {}", output_path.display());

    Ok(())
}
