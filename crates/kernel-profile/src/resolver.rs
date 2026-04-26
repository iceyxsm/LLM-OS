use crate::model::{MemoryProfile, ProfileSet};

/// Resolve a named profile from the profile set.
///
/// Returns `None` if the profile name is not found.
pub fn resolve_profile<'a>(set: &'a ProfileSet, name: &str) -> Option<&'a MemoryProfile> {
    set.profiles.get(name)
}
