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
        match after.iter().find(|candidate| candidate.name == row.name) {
            None => comparison.lost.push(row.name.clone()),
            Some(survivor) if survivor.pid.is_some() && survivor.pid == row.pid => {
                comparison.adopted.push(row.name.clone());
            }
            Some(survivor) if survivor.pid.is_some() => {
                comparison.restarted.push(row.name.clone());
            }
            Some(_) => {}
        }
    }
    comparison
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
