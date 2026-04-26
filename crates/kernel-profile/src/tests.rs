use crate::loader::parse_profiles;
use crate::model::{CgroupDefaults, MemoryProfile, OomPolicy};
use crate::resolver::resolve_profile;

const SAMPLE_TOML: &str = r#"
[profiles.balanced]
zram_fraction = 1.0
compression_algo = "zstd"
swappiness = 100

[profiles.aggressive]
zram_fraction = 1.5
compression_algo = "zstd"
swappiness = 180

[profiles.low_latency]
zram_fraction = 0.5
compression_algo = "lz4"
swappiness = 60
"#;

#[test]
fn parse_profiles_loads_all_entries() {
    let set = parse_profiles(SAMPLE_TOML).unwrap();
    assert_eq!(set.profiles.len(), 3);
    assert!(set.profiles.contains_key("balanced"));
    assert!(set.profiles.contains_key("aggressive"));
    assert!(set.profiles.contains_key("low_latency"));
}

#[test]
fn resolve_existing_profile() {
    let set = parse_profiles(SAMPLE_TOML).unwrap();
    let profile = resolve_profile(&set, "balanced").unwrap();
    assert_eq!(
        *profile,
        MemoryProfile {
            zram_fraction: 1.0,
            compression_algo: "zstd".to_string(),
            swappiness: 100,
        }
    );
}

#[test]
fn resolve_missing_profile_returns_none() {
    let set = parse_profiles(SAMPLE_TOML).unwrap();
    assert!(resolve_profile(&set, "nonexistent").is_none());
}

#[test]
fn cgroup_defaults_are_sensible() {
    let defaults = CgroupDefaults::default();
    assert_eq!(defaults.cpu_weight, 100);
    assert!(defaults.memory_max_bytes.is_none());
    assert!(defaults.memory_high_bytes.is_none());
}

#[test]
fn oom_policy_default_is_kill() {
    assert_eq!(OomPolicy::default(), OomPolicy::Kill);
}

#[test]
fn low_latency_uses_lz4() {
    let set = parse_profiles(SAMPLE_TOML).unwrap();
    let profile = resolve_profile(&set, "low_latency").unwrap();
    assert_eq!(profile.compression_algo, "lz4");
    assert_eq!(profile.swappiness, 60);
}
