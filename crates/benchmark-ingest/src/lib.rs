mod parser;
mod query;
mod record;

pub use parser::parse_benchmark_csv;
pub use query::{filter_runs, summarize_group, RunFilter, RunSummary};
pub use record::BenchmarkRecord;

#[cfg(test)]
mod tests;
