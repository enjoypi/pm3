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

fn ends_at_boundary(rest: &str) -> bool {
    if rest.is_empty() {
        return true;
    }
    rest.starts_with('/')
}

#[must_use]
pub fn covers_path(parent: &str, child: &str) -> bool {
    let parent = normalize_root(parent);
    if parent == ROOT_PATH {
        return true;
    }
    normalize_root(child)
        .strip_prefix(parent)
        .is_some_and(ends_at_boundary)
}

#[cfg(test)]
#[path = "../tests/sandbox_roots_tests.rs"]
mod tests;
