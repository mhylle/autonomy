use serde::{Deserialize, Serialize};

/// Result of ticking a behavior tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BtStatus {
    Success,
    Failure,
    Running,
}

/// Which drive to check in a CheckDrive condition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DriveKind {
    Hunger,
    Fear,
    Curiosity,
    SocialNeed,
    Aggression,
    ReproductiveUrge,
}

/// Comparison operator for condition nodes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Comparison {
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
}

impl Comparison {
    pub fn evaluate(&self, value: f64, threshold: f64) -> bool {
        match self {
            Comparison::GreaterThan => value > threshold,
            Comparison::LessThan => value < threshold,
            Comparison::GreaterOrEqual => value >= threshold,
            Comparison::LessOrEqual => value <= threshold,
        }
    }
}

/// Action produced by a behavior tree action node.
///
/// Consumed by movement/feeding/etc systems in Phase 2.4+.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtAction {
    /// Move toward the closest perceived resource.
    MoveTowardResource { speed_factor: f64 },
    /// Wander randomly.
    Wander { speed: f64 },
    /// Attempt to eat the closest adjacent resource.
    Eat,
    /// Rest (zero velocity, slight energy recovery).
    Rest,
    /// No action.
    None,
}

/// Context provided to behavior tree evaluation.
///
/// Contains read-only snapshots of the entity's state so the BT can
/// make decisions without borrowing the ECS world.
pub struct BtContext {
    /// Current drive levels.
    pub hunger: f64,
    pub fear: f64,
    pub curiosity: f64,
    pub social_need: f64,
    pub aggression: f64,
    pub reproductive_urge: f64,
    /// Current energy fraction (current / max).
    pub energy_fraction: f64,
    /// Whether any resources are perceived within sensor range.
    pub has_nearby_resource: bool,
    /// Distance to the closest perceived resource (f64::MAX if none).
    pub closest_resource_distance: f64,
}

impl BtContext {
    fn drive_value(&self, kind: &DriveKind) -> f64 {
        match kind {
            DriveKind::Hunger => self.hunger,
            DriveKind::Fear => self.fear,
            DriveKind::Curiosity => self.curiosity,
            DriveKind::SocialNeed => self.social_need,
            DriveKind::Aggression => self.aggression,
            DriveKind::ReproductiveUrge => self.reproductive_urge,
        }
    }
}

/// Behavior tree node.
///
/// Serializable with serde for genome storage and crossover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtNode {
    // -- Control flow --
    /// Run children in order; fail on first failure, succeed if all succeed.
    Sequence(Vec<BtNode>),
    /// Try children in order; succeed on first success, fail if all fail.
    Selector(Vec<BtNode>),
    /// Invert child's result (Success <-> Failure, Running unchanged).
    Inverter(Box<BtNode>),
    /// Always return Success regardless of child's result.
    AlwaysSucceed(Box<BtNode>),

    // -- Condition nodes --
    /// Check a drive against a threshold.
    CheckDrive {
        drive: DriveKind,
        threshold: f64,
        comparison: Comparison,
    },
    /// Check if there is a resource within range.
    NearbyResource { range: f64 },
    /// Check energy fraction against a threshold.
    CheckEnergy {
        threshold: f64,
        comparison: Comparison,
    },

    // -- Action nodes --
    /// Move toward the closest perceived resource.
    MoveTowardResource { speed_factor: f64 },
    /// Wander randomly.
    Wander { speed: f64 },
    /// Eat the closest adjacent resource.
    Eat,
    /// Rest (no movement).
    Rest,
}

/// Recursively evaluate a behavior tree node given a context.
///
/// Returns the status and the action produced (if any).
pub fn tick_bt(node: &BtNode, ctx: &BtContext) -> (BtStatus, BtAction) {
    match node {
        BtNode::Sequence(children) => {
            let mut last_action = BtAction::None;
            for child in children {
                let (status, action) = tick_bt(child, ctx);
                if action != BtAction::None {
                    last_action = action;
                }
                match status {
                    BtStatus::Failure => return (BtStatus::Failure, BtAction::None),
                    BtStatus::Running => return (BtStatus::Running, last_action),
                    BtStatus::Success => {}
                }
            }
            (BtStatus::Success, last_action)
        }

        BtNode::Selector(children) => {
            for child in children {
                let (status, action) = tick_bt(child, ctx);
                match status {
                    BtStatus::Success => return (BtStatus::Success, action),
                    BtStatus::Running => return (BtStatus::Running, action),
                    BtStatus::Failure => {}
                }
            }
            (BtStatus::Failure, BtAction::None)
        }

        BtNode::Inverter(child) => {
            let (status, action) = tick_bt(child, ctx);
            let inverted = match status {
                BtStatus::Success => BtStatus::Failure,
                BtStatus::Failure => BtStatus::Success,
                BtStatus::Running => BtStatus::Running,
            };
            (inverted, action)
        }

        BtNode::AlwaysSucceed(child) => {
            let (_status, action) = tick_bt(child, ctx);
            (BtStatus::Success, action)
        }

        // Condition nodes
        BtNode::CheckDrive {
            drive,
            threshold,
            comparison,
        } => {
            let value = ctx.drive_value(drive);
            if comparison.evaluate(value, *threshold) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::NearbyResource { range } => {
            if ctx.has_nearby_resource && ctx.closest_resource_distance <= *range {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::CheckEnergy {
            threshold,
            comparison,
        } => {
            if comparison.evaluate(ctx.energy_fraction, *threshold) {
                (BtStatus::Success, BtAction::None)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        // Action nodes
        BtNode::MoveTowardResource { speed_factor } => {
            if ctx.has_nearby_resource {
                (
                    BtStatus::Success,
                    BtAction::MoveTowardResource {
                        speed_factor: *speed_factor,
                    },
                )
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::Wander { speed } => (
            BtStatus::Success,
            BtAction::Wander { speed: *speed },
        ),

        BtNode::Eat => {
            if ctx.has_nearby_resource {
                (BtStatus::Success, BtAction::Eat)
            } else {
                (BtStatus::Failure, BtAction::None)
            }
        }

        BtNode::Rest => (BtStatus::Success, BtAction::Rest),
    }
}

/// Build the default starter behavior tree.
///
/// Logic: Selector(Sequence(CheckHungry, NearbyFood, MoveToFood, Eat), Wander)
pub fn default_starter_bt() -> BtNode {
    BtNode::Selector(vec![
        BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.3,
                comparison: Comparison::GreaterThan,
            },
            BtNode::NearbyResource { range: 50.0 },
            BtNode::MoveTowardResource { speed_factor: 1.0 },
            BtNode::Eat,
        ]),
        BtNode::Wander { speed: 1.5 },
    ])
}

/// Count the total number of nodes in a BT.
pub fn node_count(node: &BtNode) -> usize {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            1 + children.iter().map(node_count).sum::<usize>()
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => 1 + node_count(child),
        _ => 1, // Leaf nodes
    }
}

/// Maximum depth of a BT.
pub fn depth(node: &BtNode) -> usize {
    match node {
        BtNode::Sequence(children) | BtNode::Selector(children) => {
            1 + children.iter().map(depth).max().unwrap_or(0)
        }
        BtNode::Inverter(child) | BtNode::AlwaysSucceed(child) => 1 + depth(child),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hungry_with_food_ctx() -> BtContext {
        BtContext {
            hunger: 0.8,
            fear: 0.0,
            curiosity: 0.2,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.0,
            energy_fraction: 0.2,
            has_nearby_resource: true,
            closest_resource_distance: 15.0,
        }
    }

    fn full_no_food_ctx() -> BtContext {
        BtContext {
            hunger: 0.1,
            fear: 0.0,
            curiosity: 0.3,
            social_need: 0.0,
            aggression: 0.0,
            reproductive_urge: 0.5,
            energy_fraction: 0.9,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
        }
    }

    #[test]
    fn sequence_succeeds_when_all_succeed() {
        let bt = BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
            BtNode::Eat,
        ]);
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Eat);
    }

    #[test]
    fn sequence_fails_on_first_failure() {
        let bt = BtNode::Sequence(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Hunger,
                threshold: 0.9, // hunger is 0.8, so this fails
                comparison: Comparison::GreaterThan,
            },
            BtNode::Eat,
        ]);
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn selector_succeeds_on_first_success() {
        let bt = BtNode::Selector(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Fear,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
            BtNode::Wander { speed: 1.0 },
        ]);
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Wander { speed: 1.0 });
    }

    #[test]
    fn selector_fails_when_all_fail() {
        let bt = BtNode::Selector(vec![
            BtNode::CheckDrive {
                drive: DriveKind::Fear,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
            BtNode::CheckDrive {
                drive: DriveKind::Aggression,
                threshold: 0.5,
                comparison: Comparison::GreaterThan,
            },
        ]);
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn inverter_flips_success_to_failure() {
        let bt = BtNode::Inverter(Box::new(BtNode::Rest));
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn inverter_flips_failure_to_success() {
        let bt = BtNode::Inverter(Box::new(BtNode::CheckDrive {
            drive: DriveKind::Fear,
            threshold: 0.9,
            comparison: Comparison::GreaterThan,
        }));
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn always_succeed_wraps_failure() {
        let bt = BtNode::AlwaysSucceed(Box::new(BtNode::CheckDrive {
            drive: DriveKind::Fear,
            threshold: 0.9,
            comparison: Comparison::GreaterThan,
        }));
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
    }

    #[test]
    fn check_drive_hunger() {
        let bt = BtNode::CheckDrive {
            drive: DriveKind::Hunger,
            threshold: 0.5,
            comparison: Comparison::GreaterThan,
        };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success); // 0.8 > 0.5
    }

    #[test]
    fn check_energy_low() {
        let bt = BtNode::CheckEnergy {
            threshold: 0.3,
            comparison: Comparison::LessThan,
        };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success); // 0.2 < 0.3
    }

    #[test]
    fn nearby_resource_in_range() {
        let bt = BtNode::NearbyResource { range: 50.0 };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success); // 15.0 <= 50.0
    }

    #[test]
    fn nearby_resource_out_of_range() {
        let bt = BtNode::NearbyResource { range: 10.0 };
        let (status, _) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Failure); // 15.0 > 10.0
    }

    #[test]
    fn move_toward_resource_with_food() {
        let bt = BtNode::MoveTowardResource { speed_factor: 2.0 };
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(
            action,
            BtAction::MoveTowardResource { speed_factor: 2.0 }
        );
    }

    #[test]
    fn move_toward_resource_no_food() {
        let bt = BtNode::MoveTowardResource { speed_factor: 2.0 };
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn eat_with_food() {
        let bt = BtNode::Eat;
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Eat);
    }

    #[test]
    fn eat_no_food() {
        let bt = BtNode::Eat;
        let (status, _) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Failure);
    }

    #[test]
    fn default_bt_hungry_with_food_seeks_food() {
        let bt = default_starter_bt();
        let (status, action) = tick_bt(&bt, &hungry_with_food_ctx());
        assert_eq!(status, BtStatus::Success);
        // The sequence should produce MoveTowardResource then Eat;
        // the last action in the sequence wins.
        assert_eq!(action, BtAction::Eat);
    }

    #[test]
    fn default_bt_not_hungry_wanders() {
        let bt = default_starter_bt();
        let (status, action) = tick_bt(&bt, &full_no_food_ctx());
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Wander { speed: 1.5 });
    }

    #[test]
    fn default_bt_hungry_no_food_wanders() {
        let ctx = BtContext {
            hunger: 0.8,
            has_nearby_resource: false,
            closest_resource_distance: f64::MAX,
            ..hungry_with_food_ctx()
        };
        let bt = default_starter_bt();
        let (status, action) = tick_bt(&bt, &ctx);
        assert_eq!(status, BtStatus::Success);
        assert_eq!(action, BtAction::Wander { speed: 1.5 });
    }

    #[test]
    fn serialization_roundtrip() {
        let bt = default_starter_bt();
        let json = serde_json::to_string(&bt).unwrap();
        let deserialized: BtNode = serde_json::from_str(&json).unwrap();
        assert_eq!(bt, deserialized);
    }

    #[test]
    fn node_count_leaf() {
        assert_eq!(node_count(&BtNode::Eat), 1);
        assert_eq!(node_count(&BtNode::Rest), 1);
    }

    #[test]
    fn node_count_default_bt() {
        let bt = default_starter_bt();
        assert_eq!(node_count(&bt), 7); // Selector(Sequence(4 leaves), Wander) = 1+1+4+1
    }

    #[test]
    fn depth_leaf() {
        assert_eq!(depth(&BtNode::Eat), 1);
    }

    #[test]
    fn depth_default_bt() {
        let bt = default_starter_bt();
        assert_eq!(depth(&bt), 3); // Selector -> Sequence -> leaf
    }

    #[test]
    fn comparison_evaluate() {
        assert!(Comparison::GreaterThan.evaluate(0.5, 0.3));
        assert!(!Comparison::GreaterThan.evaluate(0.3, 0.5));
        assert!(Comparison::LessThan.evaluate(0.3, 0.5));
        assert!(!Comparison::LessThan.evaluate(0.5, 0.3));
        assert!(Comparison::GreaterOrEqual.evaluate(0.5, 0.5));
        assert!(Comparison::LessOrEqual.evaluate(0.5, 0.5));
    }
}
