use usecases::{covers_path, normalize_root};

pub(super) fn dedup_roots(candidates: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();
    for candidate in candidates {
        if !roots.contains(&candidate) {
            roots.push(candidate);
        }
    }
    roots
}

pub(super) fn dominant_roots(candidates: impl IntoIterator<Item = String>) -> Vec<String> {
    let deduped = dedup_roots(candidates);
    deduped
        .iter()
        .enumerate()
        .filter(|(index, root)| !covered_by_another(*index, root, &deduped))
        .map(|(_index, root)| root.clone())
        .collect()
}

fn covered_by_another(index: usize, root: &str, roots: &[String]) -> bool {
    for (other_index, other) in roots.iter().enumerate() {
        if other_index == index {
            continue;
        }
        if is_covered_by(index, root, other_index, other) {
            return true;
        }
    }
    false
}

fn is_covered_by(index: usize, root: &str, other_index: usize, other: &str) -> bool {
    if names_the_same_root(other, root) {
        return other_index < index;
    }
    covers_path(other, root)
}

fn names_the_same_root(other: &str, root: &str) -> bool {
    normalize_root(other) == normalize_root(root)
}

#[cfg(test)]
#[path = "../tests/apps_file_roots_tests.rs"]
mod tests;
