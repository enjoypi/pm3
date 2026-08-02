pub fn normalize_root(root: &str) -> &str {
    let trimmed = root.trim_end_matches('/');
    if trimmed.is_empty() { "/" } else { trimmed }
}

#[cfg(test)]
#[path = "../tests/sandbox_roots_tests.rs"]
mod tests;
