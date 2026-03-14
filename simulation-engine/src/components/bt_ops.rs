//! Genetic operators for behavior tree evolution.
//!
//! Provides crossover, mutation (parameter + structural), random subtree
//! generation, simplification, and depth-limit enforcement for `BtNode` trees.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::behavior_tree::{BtNode, Comparison, DriveKind, depth, node_count};

/// Maximum allowed tree depth. Trees exceeding this are rejected or pruned.
pub const MAX_DEPTH: usize = 8;

/// Standard deviation for Gaussian-like parameter mutation (fraction of value).
const PARAM_MUTATION_SIGMA: f64 = 0.15;

// ---------------------------------------------------------------------------
// Helper: enumerate and collect nodes by index
// ---------------------------------------------------------------------------

/// Count the total number of nodes in the tree (same as `node_count`).
fn count_nodes(node: &BtNode) -> usize {
    node_count(node)
}

/// Return the subtree at the given pre-order index (0-based).
fn get_subtree(node: &BtNode, target: usize) -> Option<BtNode> {
    let mut index = 0;
    get_subtree_recursive(node, target, &mut index)
}

fn get_subtree_recursive(node: &BtNode, target: usize, index: &mut usize) -> Option<BtNode> {
    if *index == target {
        return Some(node.clone());
    }
    *index += 1;

    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            for child in children {
                if let Some(result) = get_subtree_recursive(child, target, index) {
                    return Some(result);
                }
            }
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => {
            if let Some(result) = get_subtree_recursive(child, target, index) {
                return Some(result);
            }
        }
        _ => {} // leaf nodes have no children
    }
    None
}

/// Replace the subtree at the given pre-order index with `replacement`.
/// Returns the new tree.
fn replace_subtree(node: &BtNode, target: usize, replacement: &BtNode) -> BtNode {
    let mut index = 0;
    replace_subtree_recursive(node, target, replacement, &mut index)
}

fn replace_subtree_recursive(
    node: &BtNode,
    target: usize,
    replacement: &BtNode,
    index: &mut usize,
) -> BtNode {
    if *index == target {
        *index += count_nodes(node); // skip past the replaced subtree
        return replacement.clone();
    }
    *index += 1;

    match node {
        BtNode::Sequence(children) => {
            let new_children: Vec<BtNode> = children
                .iter()
                .map(|c| replace_subtree_recursive(c, target, replacement, index))
                .collect();
            BtNode::Sequence(new_children)
        }
        BtNode::Selector(children) => {
            let new_children: Vec<BtNode> = children
                .iter()
                .map(|c| replace_subtree_recursive(c, target, replacement, index))
                .collect();
            BtNode::Selector(new_children)
        }
        BtNode::Inverter(child) => {
            let new_child = replace_subtree_recursive(child, target, replacement, index);
            BtNode::Inverter(Box::new(new_child))
        }
        BtNode::AlwaysSucceed(child) => {
            let new_child = replace_subtree_recursive(child, target, replacement, index);
            BtNode::AlwaysSucceed(Box::new(new_child))
        }
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// Random leaf / subtree generation
// ---------------------------------------------------------------------------

/// All `DriveKind` variants for random selection.
const ALL_DRIVES: [DriveKind; 6] = [
    DriveKind::Hunger,
    DriveKind::Fear,
    DriveKind::Curiosity,
    DriveKind::SocialNeed,
    DriveKind::Aggression,
    DriveKind::ReproductiveUrge,
];

/// All `Comparison` variants for random selection.
const ALL_COMPARISONS: [Comparison; 4] = [
    Comparison::GreaterThan,
    Comparison::LessThan,
    Comparison::GreaterOrEqual,
    Comparison::LessOrEqual,
];

/// Generate a random leaf node (condition or action).
fn random_leaf(rng: &mut ChaCha8Rng) -> BtNode {
    match rng.gen_range(0..7) {
        0 => BtNode::CheckDrive {
            drive: ALL_DRIVES[rng.gen_range(0..ALL_DRIVES.len())],
            threshold: rng.gen_range(0.1..0.9),
            comparison: ALL_COMPARISONS[rng.gen_range(0..ALL_COMPARISONS.len())],
        },
        1 => BtNode::NearbyResource {
            range: rng.gen_range(10.0..100.0),
        },
        2 => BtNode::CheckEnergy {
            threshold: rng.gen_range(0.1..0.9),
            comparison: ALL_COMPARISONS[rng.gen_range(0..ALL_COMPARISONS.len())],
        },
        3 => BtNode::MoveTowardResource {
            speed_factor: rng.gen_range(0.5..3.0),
        },
        4 => BtNode::Wander {
            speed: rng.gen_range(0.5..4.0),
        },
        5 => BtNode::Eat,
        _ => BtNode::Rest,
    }
}

/// Grow a random behavior tree up to `max_depth`.
///
/// Uses a "grow" method: at each level, randomly choose between a leaf and
/// a branch node. At `max_depth`, always produce a leaf. The probability of
/// choosing a leaf increases as depth approaches `max_depth`.
pub fn random_subtree(rng: &mut ChaCha8Rng, max_depth: usize) -> BtNode {
    random_subtree_at_depth(rng, max_depth, 1)
}

fn random_subtree_at_depth(rng: &mut ChaCha8Rng, max_depth: usize, current_depth: usize) -> BtNode {
    // At max depth, always produce a leaf.
    if current_depth >= max_depth {
        return random_leaf(rng);
    }

    // Increasing leaf probability as we go deeper.
    let leaf_prob = current_depth as f64 / max_depth as f64;
    if rng.gen::<f64>() < leaf_prob {
        return random_leaf(rng);
    }

    // Generate a branch node.
    match rng.gen_range(0..4) {
        0 => {
            let num_children = rng.gen_range(2..=4);
            let children: Vec<BtNode> = (0..num_children)
                .map(|_| random_subtree_at_depth(rng, max_depth, current_depth + 1))
                .collect();
            BtNode::Sequence(children)
        }
        1 => {
            let num_children = rng.gen_range(2..=4);
            let children: Vec<BtNode> = (0..num_children)
                .map(|_| random_subtree_at_depth(rng, max_depth, current_depth + 1))
                .collect();
            BtNode::Selector(children)
        }
        2 => {
            let child = random_subtree_at_depth(rng, max_depth, current_depth + 1);
            BtNode::Inverter(Box::new(child))
        }
        _ => {
            let child = random_subtree_at_depth(rng, max_depth, current_depth + 1);
            BtNode::AlwaysSucceed(Box::new(child))
        }
    }
}

// ---------------------------------------------------------------------------
// Crossover
// ---------------------------------------------------------------------------

/// Perform subtree crossover between two parent trees.
///
/// Selects a random subtree from `parent_b` and inserts it at a random
/// position in `parent_a`. Returns the resulting offspring tree.
/// If the result exceeds `MAX_DEPTH`, returns a clone of `parent_a` instead.
pub fn crossover(parent_a: &BtNode, parent_b: &BtNode, rng: &mut ChaCha8Rng) -> BtNode {
    let count_a = count_nodes(parent_a);
    let count_b = count_nodes(parent_b);

    // Pick a random node in each parent.
    let insert_point = rng.gen_range(0..count_a);
    let donor_point = rng.gen_range(0..count_b);

    let donor_subtree = match get_subtree(parent_b, donor_point) {
        Some(s) => s,
        None => return parent_a.clone(),
    };

    let offspring = replace_subtree(parent_a, insert_point, &donor_subtree);

    // Enforce depth limit.
    if depth(&offspring) > MAX_DEPTH {
        parent_a.clone()
    } else {
        offspring
    }
}

// ---------------------------------------------------------------------------
// Parameter mutation
// ---------------------------------------------------------------------------

/// Apply Gaussian-like noise to all f64 parameters in the tree.
///
/// Each parameter is perturbed with probability `mutation_rate` by adding
/// uniform noise in [-sigma*value, +sigma*value], then clamped to sensible
/// bounds.
pub fn mutate_parameters(node: &BtNode, mutation_rate: f64, rng: &mut ChaCha8Rng) -> BtNode {
    match node {
        BtNode::Sequence(children) => {
            let new_children = children
                .iter()
                .map(|c| mutate_parameters(c, mutation_rate, rng))
                .collect();
            BtNode::Sequence(new_children)
        }
        BtNode::Selector(children) => {
            let new_children = children
                .iter()
                .map(|c| mutate_parameters(c, mutation_rate, rng))
                .collect();
            BtNode::Selector(new_children)
        }
        BtNode::Inverter(child) => {
            BtNode::Inverter(Box::new(mutate_parameters(child, mutation_rate, rng)))
        }
        BtNode::AlwaysSucceed(child) => {
            BtNode::AlwaysSucceed(Box::new(mutate_parameters(child, mutation_rate, rng)))
        }
        BtNode::CheckDrive {
            drive,
            threshold,
            comparison,
        } => BtNode::CheckDrive {
            drive: *drive,
            threshold: maybe_perturb(*threshold, 0.01, 1.0, mutation_rate, rng),
            comparison: *comparison,
        },
        BtNode::NearbyResource { range } => BtNode::NearbyResource {
            range: maybe_perturb(*range, 1.0, 200.0, mutation_rate, rng),
        },
        BtNode::CheckEnergy {
            threshold,
            comparison,
        } => BtNode::CheckEnergy {
            threshold: maybe_perturb(*threshold, 0.01, 1.0, mutation_rate, rng),
            comparison: *comparison,
        },
        BtNode::MoveTowardResource { speed_factor } => BtNode::MoveTowardResource {
            speed_factor: maybe_perturb(*speed_factor, 0.1, 5.0, mutation_rate, rng),
        },
        BtNode::Wander { speed } => BtNode::Wander {
            speed: maybe_perturb(*speed, 0.1, 10.0, mutation_rate, rng),
        },
        BtNode::Eat => BtNode::Eat,
        BtNode::Rest => BtNode::Rest,
    }
}

/// Possibly perturb a value with Gaussian-like noise, clamped to [min, max].
fn maybe_perturb(
    value: f64,
    min: f64,
    max: f64,
    mutation_rate: f64,
    rng: &mut ChaCha8Rng,
) -> f64 {
    if rng.gen::<f64>() < mutation_rate {
        let noise = (rng.gen::<f64>() * 2.0 - 1.0) * PARAM_MUTATION_SIGMA * value;
        (value + noise).clamp(min, max)
    } else {
        value
    }
}

// ---------------------------------------------------------------------------
// Structural mutation
// ---------------------------------------------------------------------------

/// Apply structural mutation to a behavior tree.
///
/// With probability `mutation_rate`, one of three operations is applied at
/// a randomly chosen node:
/// - **Insert**: wrap a node in a new Sequence or Selector with a random sibling.
/// - **Delete**: replace a composite/decorator node with one of its children.
/// - **Replace**: swap a leaf node for a different random leaf.
///
/// The result is checked against `MAX_DEPTH`; if it exceeds the limit,
/// the original tree is returned unchanged.
pub fn mutate_structure(node: &BtNode, mutation_rate: f64, rng: &mut ChaCha8Rng) -> BtNode {
    if rng.gen::<f64>() >= mutation_rate {
        return node.clone();
    }

    let total = count_nodes(node);
    let target = rng.gen_range(0..total);
    let target_node = match get_subtree(node, target) {
        Some(n) => n,
        None => return node.clone(),
    };

    let operation = rng.gen_range(0..3);
    let replacement = match operation {
        0 => structural_insert(&target_node, rng),
        1 => structural_delete(&target_node, rng),
        _ => structural_replace(&target_node, rng),
    };

    let result = replace_subtree(node, target, &replacement);
    if depth(&result) > MAX_DEPTH {
        node.clone()
    } else {
        result
    }
}

/// Insert: wrap the node in a Sequence or Selector with a random sibling leaf.
fn structural_insert(node: &BtNode, rng: &mut ChaCha8Rng) -> BtNode {
    let sibling = random_leaf(rng);
    if rng.gen_bool(0.5) {
        BtNode::Sequence(vec![node.clone(), sibling])
    } else {
        BtNode::Selector(vec![node.clone(), sibling])
    }
}

/// Delete: if the node is a composite or decorator, replace it with a child.
/// If it is a leaf, return it unchanged.
fn structural_delete(node: &BtNode, rng: &mut ChaCha8Rng) -> BtNode {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) if !children.is_empty() => {
            let idx = rng.gen_range(0..children.len());
            children[idx].clone()
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => *child.clone(),
        other => other.clone(),
    }
}

/// Replace: swap a leaf for a different random leaf, or swap a node type.
fn structural_replace(node: &BtNode, rng: &mut ChaCha8Rng) -> BtNode {
    match node {
        // For leaves, generate a new random leaf.
        BtNode::CheckDrive { .. }
        | BtNode::NearbyResource { .. }
        | BtNode::CheckEnergy { .. }
        | BtNode::MoveTowardResource { .. }
        | BtNode::Wander { .. }
        | BtNode::Eat
        | BtNode::Rest => random_leaf(rng),
        // For composite nodes, swap Sequence <-> Selector.
        BtNode::Sequence(children) => BtNode::Selector(children.clone()),
        BtNode::Selector(children) => BtNode::Sequence(children.clone()),
        // For decorators, swap Inverter <-> AlwaysSucceed.
        BtNode::Inverter(child) => BtNode::AlwaysSucceed(child.clone()),
        BtNode::AlwaysSucceed(child) => BtNode::Inverter(child.clone()),
    }
}

// ---------------------------------------------------------------------------
// Simplification
// ---------------------------------------------------------------------------

/// Simplify a behavior tree by removing redundant structure.
///
/// Applies the following rules recursively:
/// - Single-child Sequence or Selector is replaced by its child.
/// - Inverter(Inverter(x)) collapses to x.
/// - AlwaysSucceed(AlwaysSucceed(x)) collapses to AlwaysSucceed(x).
/// - Empty Sequence returns `Rest` (neutral action).
/// - Empty Selector returns `Rest`.
/// - Flatten nested Sequences: Sequence([..., Sequence(inner), ...]) -> Sequence([..., inner..., ...]).
/// - Flatten nested Selectors similarly.
pub fn simplify(node: &BtNode) -> BtNode {
    match node {
        BtNode::Sequence(children) => {
            // Recursively simplify children first.
            let simplified: Vec<BtNode> = children.iter().map(simplify).collect();
            // Flatten nested Sequences.
            let flattened = flatten_composite(&simplified, true);
            match flattened.len() {
                0 => BtNode::Rest,
                1 => flattened.into_iter().next().unwrap(),
                _ => BtNode::Sequence(flattened),
            }
        }
        BtNode::Selector(children) => {
            let simplified: Vec<BtNode> = children.iter().map(simplify).collect();
            let flattened = flatten_composite(&simplified, false);
            match flattened.len() {
                0 => BtNode::Rest,
                1 => flattened.into_iter().next().unwrap(),
                _ => BtNode::Selector(flattened),
            }
        }
        BtNode::Inverter(child) => {
            let simplified_child = simplify(child);
            // Inverter(Inverter(x)) -> x
            if let BtNode::Inverter(inner) = simplified_child {
                *inner
            } else {
                BtNode::Inverter(Box::new(simplified_child))
            }
        }
        BtNode::AlwaysSucceed(child) => {
            let simplified_child = simplify(child);
            // AlwaysSucceed(AlwaysSucceed(x)) -> AlwaysSucceed(x)
            if let BtNode::AlwaysSucceed(_) = &simplified_child {
                simplified_child
            } else {
                BtNode::AlwaysSucceed(Box::new(simplified_child))
            }
        }
        // Leaves are already simple.
        other => other.clone(),
    }
}

/// Flatten nested same-type composites.
/// If `is_sequence` is true, flattens nested Sequences; otherwise Selectors.
fn flatten_composite(children: &[BtNode], is_sequence: bool) -> Vec<BtNode> {
    let mut result = Vec::new();
    for child in children {
        match child {
            BtNode::Sequence(inner) if is_sequence => {
                result.extend(inner.iter().cloned());
            }
            BtNode::Selector(inner) if !is_sequence => {
                result.extend(inner.iter().cloned());
            }
            other => result.push(other.clone()),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Depth-limit enforcement
// ---------------------------------------------------------------------------

/// Check whether a tree respects the depth limit.
pub fn within_depth_limit(node: &BtNode) -> bool {
    depth(node) <= MAX_DEPTH
}

/// Prune a tree to fit within `MAX_DEPTH` by replacing deep subtrees
/// with random leaves.
pub fn enforce_depth_limit(node: &BtNode, rng: &mut ChaCha8Rng) -> BtNode {
    enforce_depth_at(node, 1, rng)
}

fn enforce_depth_at(node: &BtNode, current_depth: usize, rng: &mut ChaCha8Rng) -> BtNode {
    if current_depth >= MAX_DEPTH {
        // At or beyond max depth, replace with a leaf.
        return match node {
            // If already a leaf, keep it.
            BtNode::CheckDrive { .. }
            | BtNode::NearbyResource { .. }
            | BtNode::CheckEnergy { .. }
            | BtNode::MoveTowardResource { .. }
            | BtNode::Wander { .. }
            | BtNode::Eat
            | BtNode::Rest => node.clone(),
            // Otherwise, replace with random leaf.
            _ => random_leaf(rng),
        };
    }

    match node {
        BtNode::Sequence(children) => {
            let new_children = children
                .iter()
                .map(|c| enforce_depth_at(c, current_depth + 1, rng))
                .collect();
            BtNode::Sequence(new_children)
        }
        BtNode::Selector(children) => {
            let new_children = children
                .iter()
                .map(|c| enforce_depth_at(c, current_depth + 1, rng))
                .collect();
            BtNode::Selector(new_children)
        }
        BtNode::Inverter(child) => {
            BtNode::Inverter(Box::new(enforce_depth_at(child, current_depth + 1, rng)))
        }
        BtNode::AlwaysSucceed(child) => {
            BtNode::AlwaysSucceed(Box::new(enforce_depth_at(child, current_depth + 1, rng)))
        }
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate that a `BtNode` tree is structurally sound:
/// - depth <= MAX_DEPTH
/// - all f64 fields are finite and non-negative
/// - all composite nodes have at least 1 child
pub fn is_valid(node: &BtNode) -> bool {
    within_depth_limit(node) && is_valid_recursive(node)
}

fn is_valid_recursive(node: &BtNode) -> bool {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            !children.is_empty() && children.iter().all(is_valid_recursive)
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => is_valid_recursive(child),
        BtNode::CheckDrive { threshold, .. } => threshold.is_finite() && *threshold >= 0.0,
        BtNode::NearbyResource { range } => range.is_finite() && *range >= 0.0,
        BtNode::CheckEnergy { threshold, .. } => threshold.is_finite() && *threshold >= 0.0,
        BtNode::MoveTowardResource { speed_factor } => {
            speed_factor.is_finite() && *speed_factor >= 0.0
        }
        BtNode::Wander { speed } => speed.is_finite() && *speed >= 0.0,
        BtNode::Eat | BtNode::Rest => true,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::behavior_tree::default_starter_bt;
    use rand::SeedableRng;

    fn make_rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    // -- Random subtree generation --

    #[test]
    fn random_subtree_respects_max_depth() {
        let mut rng = make_rng(42);
        for _ in 0..200 {
            let tree = random_subtree(&mut rng, MAX_DEPTH);
            assert!(
                depth(&tree) <= MAX_DEPTH,
                "random_subtree exceeded MAX_DEPTH: depth={}",
                depth(&tree)
            );
        }
    }

    #[test]
    fn random_subtree_produces_valid_trees() {
        let mut rng = make_rng(123);
        for _ in 0..200 {
            let tree = random_subtree(&mut rng, MAX_DEPTH);
            assert!(is_valid(&tree), "random_subtree produced invalid tree");
        }
    }

    #[test]
    fn random_subtree_depth_1_is_leaf() {
        let mut rng = make_rng(999);
        for _ in 0..50 {
            let tree = random_subtree(&mut rng, 1);
            assert_eq!(depth(&tree), 1, "depth-1 subtree should be a leaf");
        }
    }

    // -- Crossover --

    #[test]
    fn crossover_produces_valid_tree() {
        let mut rng = make_rng(7);
        let parent_a = default_starter_bt();
        let parent_b = random_subtree(&mut rng, 4);

        for _ in 0..100 {
            let offspring = crossover(&parent_a, &parent_b, &mut rng);
            assert!(
                is_valid(&offspring),
                "crossover produced invalid tree: depth={}",
                depth(&offspring)
            );
        }
    }

    #[test]
    fn crossover_1000_random_pairs_all_valid() {
        let mut rng = make_rng(2024);
        for _ in 0..1000 {
            let a = random_subtree(&mut rng, 5);
            let b = random_subtree(&mut rng, 5);
            let offspring = crossover(&a, &b, &mut rng);
            assert!(
                is_valid(&offspring),
                "crossover produced invalid offspring: depth={}, nodes={}",
                depth(&offspring),
                node_count(&offspring)
            );
        }
    }

    #[test]
    fn crossover_at_root_replaces_entire_tree() {
        // When insert_point = 0 and donor_point = 0, the entire tree A is
        // replaced with the entire tree B. We just check the result is valid.
        let mut rng = make_rng(0);
        let a = BtNode::Eat;
        let b = BtNode::Rest;
        // With single-node trees and index 0, should swap root.
        let offspring = crossover(&a, &b, &mut rng);
        assert!(is_valid(&offspring));
    }

    // -- Parameter mutation --

    #[test]
    fn mutate_parameters_preserves_structure() {
        let mut rng = make_rng(55);
        let tree = default_starter_bt();
        let mutated = mutate_parameters(&tree, 1.0, &mut rng);
        // Same structure: same node count, same depth.
        assert_eq!(node_count(&mutated), node_count(&tree));
        assert_eq!(depth(&mutated), depth(&tree));
    }

    #[test]
    fn mutate_parameters_changes_values() {
        let mut rng = make_rng(77);
        let tree = BtNode::Wander { speed: 2.0 };
        let mut changed = false;
        for _ in 0..50 {
            let mutated = mutate_parameters(&tree, 1.0, &mut rng);
            if let BtNode::Wander { speed } = mutated {
                if (speed - 2.0).abs() > f64::EPSILON {
                    changed = true;
                    break;
                }
            }
        }
        assert!(changed, "parameter mutation with rate=1.0 should change values");
    }

    #[test]
    fn mutate_parameters_rate_zero_no_change() {
        let mut rng = make_rng(88);
        let tree = default_starter_bt();
        let mutated = mutate_parameters(&tree, 0.0, &mut rng);
        assert_eq!(tree, mutated, "rate=0.0 should produce identical tree");
    }

    // -- Structural mutation --

    #[test]
    fn mutate_structure_produces_valid_tree() {
        let mut rng = make_rng(33);
        for _ in 0..200 {
            let tree = random_subtree(&mut rng, 4);
            let mutated = mutate_structure(&tree, 1.0, &mut rng);
            assert!(
                is_valid(&mutated),
                "structural mutation produced invalid tree: depth={}",
                depth(&mutated)
            );
        }
    }

    #[test]
    fn mutate_1000_trees_all_valid() {
        let mut rng = make_rng(2025);
        for _ in 0..1000 {
            let tree = random_subtree(&mut rng, 5);
            // Apply both parameter and structural mutation.
            let mutated = mutate_parameters(&tree, 0.3, &mut rng);
            let mutated = mutate_structure(&mutated, 0.5, &mut rng);
            assert!(
                is_valid(&mutated),
                "mutation pipeline produced invalid tree: depth={}, nodes={}",
                depth(&mutated),
                node_count(&mutated)
            );
        }
    }

    #[test]
    fn mutate_structure_rate_zero_no_change() {
        let mut rng = make_rng(44);
        let tree = default_starter_bt();
        let mutated = mutate_structure(&tree, 0.0, &mut rng);
        assert_eq!(tree, mutated, "rate=0.0 should produce identical tree");
    }

    // -- Simplification --

    #[test]
    fn simplify_single_child_sequence() {
        let tree = BtNode::Sequence(vec![BtNode::Eat]);
        let simplified = simplify(&tree);
        assert_eq!(simplified, BtNode::Eat);
    }

    #[test]
    fn simplify_single_child_selector() {
        let tree = BtNode::Selector(vec![BtNode::Rest]);
        let simplified = simplify(&tree);
        assert_eq!(simplified, BtNode::Rest);
    }

    #[test]
    fn simplify_double_inverter() {
        let tree = BtNode::Inverter(Box::new(BtNode::Inverter(Box::new(BtNode::Eat))));
        let simplified = simplify(&tree);
        assert_eq!(simplified, BtNode::Eat);
    }

    #[test]
    fn simplify_double_always_succeed() {
        let tree = BtNode::AlwaysSucceed(Box::new(BtNode::AlwaysSucceed(Box::new(BtNode::Eat))));
        let simplified = simplify(&tree);
        assert_eq!(simplified, BtNode::AlwaysSucceed(Box::new(BtNode::Eat)));
    }

    #[test]
    fn simplify_nested_sequences_flatten() {
        let tree = BtNode::Sequence(vec![
            BtNode::Eat,
            BtNode::Sequence(vec![BtNode::Rest, BtNode::Eat]),
        ]);
        let simplified = simplify(&tree);
        assert_eq!(
            simplified,
            BtNode::Sequence(vec![BtNode::Eat, BtNode::Rest, BtNode::Eat])
        );
    }

    #[test]
    fn simplify_empty_sequence() {
        let tree = BtNode::Sequence(vec![]);
        let simplified = simplify(&tree);
        assert_eq!(simplified, BtNode::Rest);
    }

    // -- Depth limit --

    #[test]
    fn enforce_depth_limit_prunes_deep_tree() {
        // Build a tree deeper than MAX_DEPTH.
        let mut tree = BtNode::Eat;
        for _ in 0..MAX_DEPTH + 3 {
            tree = BtNode::Inverter(Box::new(tree));
        }
        assert!(depth(&tree) > MAX_DEPTH);

        let mut rng = make_rng(11);
        let pruned = enforce_depth_limit(&tree, &mut rng);
        assert!(
            depth(&pruned) <= MAX_DEPTH,
            "pruned tree should be within MAX_DEPTH: depth={}",
            depth(&pruned)
        );
    }

    #[test]
    fn within_depth_limit_accepts_valid_tree() {
        assert!(within_depth_limit(&default_starter_bt()));
    }

    #[test]
    fn within_depth_limit_rejects_deep_tree() {
        let mut tree = BtNode::Eat;
        for _ in 0..MAX_DEPTH + 1 {
            tree = BtNode::Inverter(Box::new(tree));
        }
        assert!(!within_depth_limit(&tree));
    }

    // -- Validation --

    #[test]
    fn default_bt_is_valid() {
        assert!(is_valid(&default_starter_bt()));
    }

    #[test]
    fn invalid_tree_negative_threshold() {
        let tree = BtNode::CheckDrive {
            drive: DriveKind::Hunger,
            threshold: -0.5,
            comparison: Comparison::GreaterThan,
        };
        assert!(!is_valid(&tree));
    }

    #[test]
    fn invalid_tree_nan_speed() {
        let tree = BtNode::Wander { speed: f64::NAN };
        assert!(!is_valid(&tree));
    }

    // -- Get/replace subtree helpers --

    #[test]
    fn get_and_replace_subtree_roundtrip() {
        let tree = default_starter_bt();
        let total = count_nodes(&tree);
        // Replacing every node with itself should yield the same tree.
        for i in 0..total {
            let sub = get_subtree(&tree, i).unwrap();
            let rebuilt = replace_subtree(&tree, i, &sub);
            assert_eq!(tree, rebuilt, "roundtrip failed at index {i}");
        }
    }
}
