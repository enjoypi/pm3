#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSnapshot {
    pub name: String,
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HandoverComparison {
    pub adopted: Vec<String>,
    pub restarted: Vec<String>,
    pub lost: Vec<String>,
}

#[must_use]
pub fn compare_handover(
    before: &[ServiceSnapshot],
    after: &[ServiceSnapshot],
) -> HandoverComparison {
    let mut comparison = HandoverComparison::default();
    for row in before {
        let survivor = after.iter().find(|candidate| candidate.name == row.name);
        match classify_handover(row.pid, survivor) {
            HandoverChange::Adopted => comparison.adopted.push(row.name.clone()),
            HandoverChange::Restarted => comparison.restarted.push(row.name.clone()),
            HandoverChange::Lost => comparison.lost.push(row.name.clone()),
            HandoverChange::Idle => {}
        }
    }
    comparison
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum HandoverChange {
    Adopted,
    Restarted,
    Lost,
    Idle,
}

const fn classify_handover(before: Option<u32>, after: Option<&ServiceSnapshot>) -> HandoverChange {
    let Some(survivor) = after else {
        return HandoverChange::Lost;
    };
    match (before, survivor.pid) {
        (Some(had), Some(has)) if had == has => HandoverChange::Adopted,
        (_, Some(_)) => HandoverChange::Restarted,
        (Some(_), None) => HandoverChange::Lost,
        (None, None) => HandoverChange::Idle,
    }
}

#[must_use]
pub fn describe_handover(comparison: &HandoverComparison) -> String {
    let HandoverComparison {
        adopted,
        restarted,
        lost,
    } = comparison;
    if adopted.len() + restarted.len() + lost.len() == 0 {
        return "no managed services to reclaim".to_owned();
    }
    [
        describe_group("adopted", adopted),
        describe_group("restarted", restarted),
        describe_group("lost", lost),
    ]
    .into_iter()
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn describe_group(label: &str, names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    format!("{label} {}: {}", names.len(), names.join(", "))
}

#[cfg(test)]
#[path = "tests/handover_tests.rs"]
mod tests;
