use anyhow::{Result, bail};
use rayon::prelude::*;
use std::path::Path;
use walkdir::WalkDir;

pub fn find_rust_files(path: &str) -> Result<Vec<String>> {
    let path = Path::new(path);

    if path.is_file() {
        if let Some(extension) = path.extension()
            && extension == "rs"
        {
            return Ok(vec![path.to_string_lossy().to_string()]);
        }
        Ok(Vec::new())
    } else if path.is_dir() {
        // collect all .rs files in parallel, excluding target directories

        let rust_files: Vec<String> = WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                // skip target directories entirely

                let is_target = e
                    .file_name()
                    .to_str()
                    .map(|s| s == "target")
                    .unwrap_or(false);
                !is_target
            })
            .par_bridge()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let file_path = entry.path();

                // only process files with .rs extension

                if file_path.is_file()
                    && let Some(extension) = file_path.extension()
                    && extension == "rs"
                {
                    return Some(file_path.to_string_lossy().into_owned());
                }
                None
            })
            .collect();

        Ok(rust_files)
    } else {
        bail!("path '{}' does not exist", path.display());
    }
}
