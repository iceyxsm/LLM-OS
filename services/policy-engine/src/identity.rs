use llmos_identity::WorkloadId;

/// Validate that a subject string is a well-formed workload identifier.
///
/// Returns the parsed WorkloadId on success, or an error message on failure.
/// This is used at policy evaluation time to provide better diagnostics
/// when subjects do not follow the namespace/name convention.
pub fn validate_subject(subject: &str) -> Result<WorkloadId, String> {
    WorkloadId::parse(subject).map_err(|e| e.to_string())
}

/// Check whether a subject string matches a policy rule subject pattern.
///
/// Supports exact matches and wildcard prefix matches (e.g. "runtime/*"
/// matches "runtime/model-runtime"). Falls back to raw string comparison
/// if the pattern is not a valid workload id (to preserve backward
/// compatibility with existing policy documents).
pub fn subject_matches(pattern: &str, subject: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix("/*") {
        if let Ok(id) = WorkloadId::parse(subject) {
            return id.namespace() == prefix;
        }
        return subject.starts_with(&format!("{prefix}/"));
    }

    pattern == subject
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_valid_subject() {
        let id = validate_subject("runtime/model-runtime").unwrap();
        assert_eq!(id.namespace(), "runtime");
        assert_eq!(id.name(), "model-runtime");
    }

    #[test]
    fn validate_invalid_subject() {
        assert!(validate_subject("no-slash").is_err());
    }

    #[test]
    fn exact_match() {
        assert!(subject_matches("runtime/model-runtime", "runtime/model-runtime"));
        assert!(!subject_matches("runtime/model-runtime", "runtime/mcp-runtime"));
    }

    #[test]
    fn wildcard_star_matches_everything() {
        assert!(subject_matches("*", "runtime/model-runtime"));
        assert!(subject_matches("*", "security/audit"));
    }

    #[test]
    fn namespace_wildcard_matches_same_namespace() {
        assert!(subject_matches("runtime/*", "runtime/model-runtime"));
        assert!(subject_matches("runtime/*", "runtime/mcp-runtime"));
        assert!(!subject_matches("runtime/*", "security/audit"));
    }

    #[test]
    fn backward_compatible_with_raw_strings() {
        // Even if the subject is not a valid workload id, prefix matching
        // still works via string comparison.
        assert!(subject_matches("runtime/*", "runtime/something"));
    }
}
