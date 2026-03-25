use std::any::TypeId;
use std::collections::{HashMap, HashSet};

use crate::di::error::DiError;

/// Node in the dependency graph.
struct Node {
    pub type_name: &'static str,
    pub dependencies: Vec<TypeId>,
}

/// Represents the DI dependency graph.
pub struct DependencyGraph {
    nodes: HashMap<TypeId, Node>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Add a provider to the graph.
    pub fn add_node(
        &mut self,
        type_id: TypeId,
        type_name: &'static str,
        dependencies: Vec<TypeId>,
    ) {
        self.nodes.insert(
            type_id,
            Node {
                type_name,
                dependencies,
            },
        );
    }

    /// Run a topological sort to determine initialization order and detect cycles.
    /// Returns an ordered list of TypeIds, from those with no dependencies up to the root.
    pub fn resolve_order(&self) -> Result<Vec<TypeId>, DiError> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        for id in self.nodes.keys() {
            if !visited.contains(id) {
                let mut path = Vec::new();
                self.dfs(*id, &mut visited, &mut visiting, &mut order, &mut path)?;
            }
        }

        Ok(order)
    }

    fn dfs(
        &self,
        current: TypeId,
        visited: &mut HashSet<TypeId>,
        visiting: &mut HashSet<TypeId>,
        order: &mut Vec<TypeId>,
        path: &mut Vec<&'static str>,
    ) -> Result<(), DiError> {
        // If the current node is not registered, we can't process it here.
        // The container handles missing provider errors.
        if let Some(node) = self.nodes.get(&current) {
            path.push(node.type_name);

            if visiting.contains(&current) {
                // Cycle detected
                let cycle = path.join(" -> ");
                return Err(DiError::CircularDependency(cycle));
            }

            if !visited.contains(&current) {
                visiting.insert(current);

                for dep in &node.dependencies {
                    self.dfs(*dep, visited, visiting, order, path)?;
                }

                visiting.remove(&current);
                visited.insert(current);
                order.push(current);
            }

            path.pop();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct A;
    struct B;
    struct C;
    struct D;

    #[test]
    fn test_valid_graph_resolution() {
        let mut graph = DependencyGraph::new();
        graph.add_node(
            TypeId::of::<A>(),
            "A",
            vec![TypeId::of::<B>(), TypeId::of::<C>()],
        );
        graph.add_node(TypeId::of::<B>(), "B", vec![TypeId::of::<D>()]);
        graph.add_node(TypeId::of::<C>(), "C", vec![TypeId::of::<D>()]);
        graph.add_node(TypeId::of::<D>(), "D", vec![]);

        let order = graph.resolve_order().unwrap();

        // D must come first
        assert_eq!(order[0], TypeId::of::<D>());
        // A must come last
        assert_eq!(order[3], TypeId::of::<A>());
    }

    #[test]
    fn test_circular_dependency() {
        let mut graph = DependencyGraph::new();
        graph.add_node(TypeId::of::<A>(), "A", vec![TypeId::of::<B>()]);
        graph.add_node(TypeId::of::<B>(), "B", vec![TypeId::of::<C>()]);
        graph.add_node(TypeId::of::<C>(), "C", vec![TypeId::of::<A>()]);

        let result = graph.resolve_order();
        assert!(result.is_err());

        if let Err(DiError::CircularDependency(msg)) = result {
            assert!(
                msg.contains("A -> B -> C -> A")
                    || msg.contains("B -> C -> A -> B")
                    || msg.contains("C -> A -> B -> C")
            );
        } else {
            panic!("Expected CircularDependency error");
        }
    }
}
