const ROOT_PATH: &str = "/";

#[must_use]
pub fn normalize_root(root: &str) -> &str {
    let trimmed = root.trim_end_matches('/');
    if trimmed.is_empty() {
        ROOT_PATH
    } else {
        trimmed
    }
}

#[must_use]
pub fn covers_path(parent: &str, child: &str) -> bool {
    let parent = normalize_root(parent);
    if parent == ROOT_PATH {
        return true;
    }
    normalize_root(child)
        .strip_prefix(parent)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

#[cfg(test)]
#[path = "../tests/sandbox_roots_tests.rs"]
mod tests;
