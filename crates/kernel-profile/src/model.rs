use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A named collection of kernel profiles loaded from configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileSet {
    pub profiles: HashMap<String, MemoryProfile>,
}

/// Memory compression and swap settings for a workload profile.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MemoryProfile {
    /// Fraction of physical RAM to allocate for zram (e.g. 1.0 = 100%).
    pub zram_fraction: f64,
    /// Compression algorithm (e.g. "zstd", "lz4").
    pub compression_algo: String,
    /// Kernel swappiness value (0-200 on modern kernels).
    pub swappiness: u32,
}

/// cgroup v2 defaults for LLM workloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CgroupDefaults {
    /// Memory limit in bytes. None means no limit.
    pub memory_max_bytes: Option<u64>,
    /// Memory high watermark in bytes. None means no watermark.
    pub memory_high_bytes: Option<u64>,
    /// CPU weight (1-10000, default 100).
    pub cpu_weight: u32,
}

impl Default for CgroupDefaults {
    fn default() -> Self {
        Self {
            memory_max_bytes: None,
            memory_high_bytes: None,
            cpu_weight: 100,
        }
    }
}

/// OOM policy recommendation for a workload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OomPolicy {
    /// Kill the workload process on OOM.
    #[default]
    Kill,
    /// Pause the workload and alert the operator.
    Pause,
    /// Attempt to reclaim memory by compressing before killing.
    CompressThenKill,
}
