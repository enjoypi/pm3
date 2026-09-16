use usecases::covers_path;

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
        .filter(|root| !covered_by_another(root, &deduped))
        .cloned()
        .collect()
}

fn covered_by_another(root: &str, roots: &[String]) -> bool {
    roots.iter().any(|other| {
        if other == root {
            return false;
        }
        covers_path(other, root)
    })
}

#[cfg(test)]
#[path = "../tests/apps_file_roots_tests.rs"]
mod tests;
