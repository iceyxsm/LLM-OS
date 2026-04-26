use std::path::Path;

use anyhow::{Context, Result};

use crate::model::ProfileSet;

/// Load a set of memory profiles from a TOML file.
///
/// The expected format matches `config/memory-profiles.toml`:
///
/// ```toml
/// [profiles.balanced]
/// zram_fraction = 1.0
/// compression_algo = "zstd"
/// swappiness = 100
/// ```
pub fn load_profiles(path: &Path) -> Result<ProfileSet> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read profile file at {}", path.display()))?;
    parse_profiles(&content)
}

/// Parse profile TOML from a string.
pub fn parse_profiles(content: &str) -> Result<ProfileSet> {
    let set: ProfileSet =
        toml::from_str(content).context("failed to parse memory profiles TOML")?;
    Ok(set)
}
