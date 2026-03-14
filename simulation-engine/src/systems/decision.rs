use crate::components::action::Action;
use crate::components::behavior_tree::{tick_bt, BtAction, BtContext, BtNode};
use crate::components::drives::Drives;
use crate::components::perception::Perception;
use crate::components::physical::Energy;
use crate::components::spatial::Position;
use crate::core::world::SimulationWorld;

/// Ticks each entity's behavior tree and produces an `Action` component.
///
/// Builds a `BtContext` from the entity's current state, evaluates the BT,
/// and converts the resulting `BtAction` into a concrete `Action` with
/// world-space coordinates (e.g. MoveTowardResource -> MoveTo closest resource).
pub fn run(world: &mut SimulationWorld) {
    let mut rng = world.rng.tick_rng("decision", world.tick);

    // Collect entity state for BT evaluation.
    let evaluations: Vec<_> = world
        .ecs
        .query::<(&Position, &Energy, &Drives, &Perception, &BtNode)>()
        .iter()
        .map(|(entity, (pos, energy, drives, perception, bt))| {
            let energy_fraction = if energy.max > 0.0 {
                energy.current / energy.max
            } else {
                0.0
            };

            let closest_res = perception.closest_resource();
            let has_nearby_resource = closest_res.is_some();
            let closest_resource_distance = closest_res
                .map(|r| r.distance)
                .unwrap_or(f64::MAX);

            let ctx = BtContext {
                hunger: drives.hunger,
                fear: drives.fear,
                curiosity: drives.curiosity,
                social_need: drives.social_need,
                aggression: drives.aggression,
                reproductive_urge: drives.reproductive_urge,
                energy_fraction,
                has_nearby_resource,
                closest_resource_distance,
            };

            let (_status, bt_action) = tick_bt(bt, &ctx);

            // Convert BtAction to Action with concrete coordinates.
            let action = match bt_action {
                BtAction::MoveTowardResource { speed_factor } => {
                    if let Some(res) = closest_res {
                        Action::MoveTo {
                            x: res.x,
                            y: res.y,
                            speed: speed_factor * 2.0, // base speed * factor
                        }
                    } else {
                        Action::Wander { speed: 1.5 }
                    }
                }
                BtAction::Wander { speed } => Action::Wander { speed },
                BtAction::Eat => Action::Eat,
                BtAction::Rest => Action::Rest,
                BtAction::None => Action::None,
            };

            (entity, action, pos.x, pos.y)
        })
        .collect();

    // Apply actions.
    for (entity, action, _x, _y) in evaluations {
        if let Ok(mut a) = world.ecs.get::<&mut Action>(entity) {
            *a = action;
        }
    }

    // For entities with Action but no BT, set Wander as fallback.
    let no_bt: Vec<_> = world
        .ecs
        .query::<&Action>()
        .without::<&BtNode>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for entity in no_bt {
        if let Ok(mut a) = world.ecs.get::<&mut Action>(entity) {
            let speed: f64 = rand::Rng::gen_range(&mut rng, 1.0..2.0);
            *a = Action::Wander { speed };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::behavior_tree::{default_starter_bt, BtNode};
    use crate::components::drives::Drives;
    use crate::components::perception::{PerceivedResource, Perception};
    use crate::core::config::SimulationConfig;

    fn test_world() -> SimulationWorld {
        SimulationWorld::new(SimulationConfig::default())
    }

    fn spawn_bt_entity(
        world: &mut SimulationWorld,
        energy_current: f64,
        energy_max: f64,
        bt: BtNode,
        perception: Perception,
    ) -> hecs::Entity {
        let drives = Drives {
            hunger: 1.0 - (energy_current / energy_max),
            ..Drives::default()
        };
        world.ecs.spawn((
            Position { x: 50.0, y: 50.0 },
            Energy {
                current: energy_current,
                max: energy_max,
                metabolism_rate: 0.1,
            },
            drives,
            perception,
            bt,
            Action::None,
        ))
    }

    #[test]
    fn hungry_entity_with_food_seeks_food() {
        let mut world = test_world();
        let perception = Perception {
            perceived_entities: vec![],
            perceived_resources: vec![PerceivedResource {
                resource_index: 0,
                x: 80.0,
                y: 50.0,
                distance: 30.0,
            }],
        };
        let e = spawn_bt_entity(&mut world, 20.0, 100.0, default_starter_bt(), perception);

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::MoveTo { x, y, .. } => {
                assert_eq!(*x, 80.0);
                assert_eq!(*y, 50.0);
            }
            Action::Eat => {} // also valid (if already adjacent)
            other => panic!("expected MoveTo or Eat, got {:?}", other),
        }
    }

    #[test]
    fn full_entity_wanders() {
        let mut world = test_world();
        let e = spawn_bt_entity(
            &mut world,
            95.0,
            100.0,
            default_starter_bt(),
            Perception::default(),
        );

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        match &*action {
            Action::Wander { speed } => {
                assert_eq!(*speed, 1.5);
            }
            other => panic!("expected Wander, got {:?}", other),
        }
    }

    #[test]
    fn entity_with_rest_bt_rests() {
        let mut world = test_world();
        let bt = BtNode::Rest;
        let e = spawn_bt_entity(&mut world, 50.0, 100.0, bt, Perception::default());

        run(&mut world);

        let action = world.ecs.get::<&Action>(e).unwrap();
        assert_eq!(*action, Action::Rest);
    }

    #[test]
    fn action_updates_each_tick() {
        let mut world = test_world();
        let e = spawn_bt_entity(
            &mut world,
            95.0,
            100.0,
            default_starter_bt(),
            Perception::default(),
        );

        run(&mut world);
        let action1 = (*world.ecs.get::<&Action>(e).unwrap()).clone();

        // Make entity hungry.
        world.ecs.get::<&mut Energy>(e).unwrap().current = 20.0;
        world.ecs.get::<&mut Drives>(e).unwrap().hunger = 0.8;

        run(&mut world);
        let action2 = (*world.ecs.get::<&Action>(e).unwrap()).clone();

        // First tick should be Wander, second should still be Wander
        // (no food perceived), but action should have been re-evaluated.
        assert_eq!(action1, Action::Wander { speed: 1.5 });
        assert_eq!(action2, Action::Wander { speed: 1.5 });
    }
}
