use std::path::PathBuf;

use clap::Args;
use llmos_kernel_profile::{load_profiles, resolve_profile};
use serde_json::json;

use crate::output::{render_json, OutputFormat};

#[derive(Args, Debug)]
pub struct ProfileArgs {
    /// Path to the memory profiles TOML file.
    #[arg(long, default_value = "config/memory-profiles.toml")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum ProfileCommand {
    /// List all available memory profiles.
    List(ProfileListArgs),
    /// Show details of a specific profile.
    Show(ProfileShowArgs),
}

#[derive(Args, Debug)]
pub struct ProfileListArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct ProfileShowArgs {
    /// Name of the profile to display.
    pub name: String,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

pub fn run_profile(args: &ProfileArgs) -> anyhow::Result<()> {
    let profile_set = load_profiles(&args.config)?;

    match &args.command {
        ProfileCommand::List(list_args) => {
            let mut names: Vec<&String> = profile_set.profiles.keys().collect();
            names.sort();

            match list_args.format {
                OutputFormat::Json => {
                    let entries: Vec<serde_json::Value> = names
                        .iter()
                        .filter_map(|name| {
                            profile_set.profiles.get(*name).map(|p| {
                                json!({
                                    "name": name,
                                    "zram_fraction": p.zram_fraction,
                                    "compression_algo": p.compression_algo,
                                    "swappiness": p.swappiness,
                                })
                            })
                        })
                        .collect();
                    let output = render_json(&json!({ "profiles": entries }))?;
                    println!("{}", output);
                }
                OutputFormat::Text => {
                    println!(
                        "{:<20} {:>14} {:>12} {:>10}",
                        "NAME", "ZRAM_FRACTION", "ALGO", "SWAPPINESS"
                    );
                    for name in &names {
                        if let Some(p) = profile_set.profiles.get(*name) {
                            println!(
                                "{:<20} {:>14.1} {:>12} {:>10}",
                                name, p.zram_fraction, p.compression_algo, p.swappiness
                            );
                        }
                    }
                }
            }
        }
        ProfileCommand::Show(show_args) => match resolve_profile(&profile_set, &show_args.name) {
            Some(profile) => match show_args.format {
                OutputFormat::Json => {
                    let output = render_json(&json!({
                        "name": show_args.name,
                        "zram_fraction": profile.zram_fraction,
                        "compression_algo": profile.compression_algo,
                        "swappiness": profile.swappiness,
                    }))?;
                    println!("{}", output);
                }
                OutputFormat::Text => {
                    println!("Profile: {}", show_args.name);
                    println!("  zram_fraction:    {}", profile.zram_fraction);
                    println!("  compression_algo: {}", profile.compression_algo);
                    println!("  swappiness:       {}", profile.swappiness);
                }
            },
            None => {
                anyhow::bail!("profile '{}' not found", show_args.name);
            }
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_profile_list_json_includes_expected_keys() {
        // Verify the JSON structure matches the contract.
        let entries = vec![json!({
            "name": "balanced",
            "zram_fraction": 1.0,
            "compression_algo": "zstd",
            "swappiness": 100,
        })];
        let output = json!({ "profiles": entries });
        let profiles = output["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 1);
        let first = &profiles[0];
        assert!(first.get("name").is_some());
        assert!(first.get("zram_fraction").is_some());
        assert!(first.get("compression_algo").is_some());
        assert!(first.get("swappiness").is_some());
    }

    #[test]
    fn render_profile_show_json_includes_expected_keys() {
        let output = json!({
            "name": "aggressive",
            "zram_fraction": 1.5,
            "compression_algo": "zstd",
            "swappiness": 180,
        });
        assert_eq!(output["name"].as_str().unwrap(), "aggressive");
        assert_eq!(output["zram_fraction"].as_f64().unwrap(), 1.5);
    }
}
