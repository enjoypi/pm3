pub(super) fn dedup_roots(candidates: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();
    for candidate in candidates {
        if !roots.contains(&candidate) {
            roots.push(candidate);
        }
    }
    roots
}

#[cfg(test)]
#[path = "../tests/apps_file_roots_tests.rs"]
mod tests;
