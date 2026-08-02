use thiserror::Error;

#[derive(Copy, Clone, Debug)]
pub struct DependencyNode<'a> {
    pub name: &'a str,
    pub depends_on: &'a [String],
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum DependencyError {
    #[error("cannot resolve dependency '{dependency}' declared by app '{app}': no such app")]
    UnknownDependency { app: String, dependency: String },

    #[error("cannot order apps: dependency cycle involves {}", involved.join(", "))]
    Cycle { involved: Vec<String> },
}

pub fn topo_sort(nodes: &[DependencyNode<'_>]) -> Result<Vec<String>, DependencyError> {
    ensure_dependencies_known(nodes)?;

    let mut order: Vec<String> = Vec::with_capacity(nodes.len());
    while order.len() < nodes.len() {
        let ready = next_ready_names(nodes, &order);
        if ready.is_empty() {
            return Err(DependencyError::Cycle {
                involved: cycle_members(nodes, &order),
            });
        }
        order.extend(ready);
    }
    Ok(order)
}

fn ensure_dependencies_known(nodes: &[DependencyNode<'_>]) -> Result<(), DependencyError> {
    for node in nodes {
        for dependency in node.depends_on {
            if !nodes.iter().any(|other| other.name == dependency.as_str()) {
                return Err(DependencyError::UnknownDependency {
                    app: node.name.to_string(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    Ok(())
}

fn next_ready_names(nodes: &[DependencyNode<'_>], order: &[String]) -> Vec<String> {
    nodes
        .iter()
        .filter(|node| !contains_name(order, node.name))
        .filter(|node| {
            node.depends_on
                .iter()
                .all(|dependency| contains_name(order, dependency))
        })
        .map(|node| node.name.to_string())
        .collect()
}

fn cycle_members(nodes: &[DependencyNode<'_>], order: &[String]) -> Vec<String> {
    nodes
        .iter()
        .filter(|node| !contains_name(order, node.name))
        .filter(|node| reaches_itself(node, nodes))
        .map(|node| node.name.to_string())
        .collect()
}

fn reaches_itself(origin: &DependencyNode<'_>, nodes: &[DependencyNode<'_>]) -> bool {
    let mut visited: Vec<&str> = Vec::new();
    let mut pending: Vec<&str> = origin.depends_on.iter().map(String::as_str).collect();
    while let Some(name) = pending.pop() {
        if name == origin.name {
            return true;
        }
        if visited.contains(&name) {
            continue;
        }
        visited.push(name);
        let node = nodes
            .iter()
            .find(|candidate| candidate.name == name)
            .expect("internal error: dependency names are verified before cycle detection");
        pending.extend(node.depends_on.iter().map(String::as_str));
    }
    false
}

fn contains_name(names: &[String], candidate: &str) -> bool {
    names.iter().any(|name| name == candidate)
}

#[cfg(test)]
#[path = "../tests/process_depgraph_tests.rs"]
mod tests;
