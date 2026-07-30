use thiserror::Error;

#[derive(Copy, Clone, Debug)]
pub struct DependencyNode<'a> {
    pub name: &'a str,
    pub depends_on: &'a [String],
}

#[derive(Debug, Error, Eq, PartialEq)]
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
                involved: unresolved_names(nodes, &order),
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

fn unresolved_names(nodes: &[DependencyNode<'_>], order: &[String]) -> Vec<String> {
    nodes
        .iter()
        .filter(|node| !contains_name(order, node.name))
        .map(|node| node.name.to_string())
        .collect()
}

fn contains_name(names: &[String], candidate: &str) -> bool {
    names.iter().any(|name| name == candidate)
}

#[cfg(test)]
#[path = "../tests/process_depgraph_tests.rs"]
mod tests;
