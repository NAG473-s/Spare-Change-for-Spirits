//! Neighbours in the market and harbour. Feet and carried tools turn at their own joints.
use crate::data::{PropDef, Scene, WalkDef};
use crate::day_cycle::Phase;
use crate::harbor::Location;
use crate::save::Settings;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use std::f32::consts::{PI, TAU};

const JOINTS: [&str; 3] = ["Hip.L", "Hip.R", "Shoulder.R"];
const STRIDE: f32 = 0.78;

pub(super) fn install(app: &mut App) {
    app.add_systems(
        Update,
        walk.run_if(crate::menu::shop_unpaused)
            .run_if(not(in_state(Phase::Menu))),
    );
}

#[derive(Component)]
pub(super) struct Resident {
    site: Location,
    start: Vec3,
    span: Vec3,
    route: WalkDef,
    elapsed: f32,
    joints: [Option<(Entity, Quat)>; 3],
}

pub(super) struct Pose {
    pub at: Transform,
    step: f32,
    arm: f32,
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

impl Resident {
    pub(super) fn new(prop: &PropDef, route: WalkDef, start: Vec3) -> Self {
        let site = match prop.scene {
            Scene::Market => Location::Market,
            Scene::Harbor => Location::Harbor,
            Scene::Shop => Location::Shop,
        };
        Self {
            site,
            start,
            span: Vec3::new(route.to.0 - prop.pos.0, 0., route.to.1 - prop.pos.2),
            route,
            elapsed: route.offset,
            joints: [None; 3],
        }
    }

    pub(super) fn pose(&self) -> Pose {
        let leg = self.route.travel + self.route.pause;
        let clock = self.elapsed.rem_euclid(2. * leg);
        let returning = clock >= leg;
        let time = if returning { clock - leg } else { clock };
        let travel = ease(time / self.route.travel);
        let stop = ((time - self.route.travel) / self.route.pause).clamp(0., 1.);
        let turn = ease((stop - 0.40) / 0.60);
        let along = if returning { 1. - travel } else { travel };
        let heading = self.span.x.atan2(self.span.z);
        // Turn across the front of the diorama so the errand is readable during the stop.
        let yaw = heading - PI.copysign(heading) * if returning { 1. - turn } else { turn };
        // Drive the soles by distance travelled; they settle before the resident turns around.
        let stride = (travel * self.span.length() * TAU / STRIDE).sin();
        let step = 0.46 * stride * (8. * travel.min(1. - travel)).min(1.);
        let gesture = (PI * stop).sin().powi(2);
        Pose {
            at: Transform::from_translation(self.start + self.span * along)
                .with_rotation(Quat::from_rotation_y(yaw)),
            step,
            arm: -0.48 * gesture - 0.30 * step,
        }
    }

    /// Give a missing model the same pivots as the finished figure.
    pub(super) fn stand_in(
        &mut self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        root: Entity,
        coat: Color,
    ) {
        let pivots = [(-0.105, 0.56), (0.105, 0.56), (0.20, 1.04)].map(|(x, y)| {
            commands
                .spawn((
                    Transform::from_xyz(x, y, 0.),
                    Visibility::default(),
                    ChildOf(root),
                ))
                .id()
        });
        self.joints = pivots.map(|joint| Some((joint, Quat::IDENTITY)));
        let coat = materials.add(coat);
        let leather = materials.add(Color::srgb(0.24, 0.20, 0.20));
        let face = materials.add(Color::srgb(0.81, 0.73, 0.61));
        let mut piece = |parent, mesh: Mesh, material: &Handle<StandardMaterial>, at| {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                at,
                ChildOf(parent),
                Pickable::IGNORE,
            ));
        };
        piece(
            root,
            Cylinder::new(0.22, 0.61).into(),
            &coat,
            Transform::from_xyz(0., 0.82, 0.),
        );
        piece(
            root,
            Sphere::new(0.17).into(),
            &face,
            Transform::from_xyz(0., 1.28, 0.),
        );
        piece(
            root,
            Cylinder::new(0.19, 0.08).into(),
            &coat,
            Transform::from_xyz(0., 1.42, 0.),
        );
        for leg in &pivots[..2] {
            piece(
                *leg,
                Cuboid::new(0.13, 0.55, 0.20).into(),
                &leather,
                Transform::from_xyz(0., -0.28, 0.02),
            );
        }
        piece(
            pivots[2],
            Cuboid::new(0.12, 0.34, 0.15).into(),
            &coat,
            Transform::from_xyz(0., -0.15, 0.03),
        );
    }
}

pub(super) fn bind_joints(
    ready: On<WorldInstanceReady>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform), Without<Mesh3d>>,
    mut residents: Query<&mut Resident>,
) {
    let Ok(parent) = parents.get(ready.entity) else {
        return;
    };
    let Ok(mut resident) = residents.get_mut(parent.parent()) else {
        return;
    };
    for child in children.iter_descendants(ready.entity) {
        if let Ok((name, at)) = nodes.get(child)
            && let Some(index) = JOINTS.iter().position(|&joint| joint == name.as_str())
        {
            resident.joints[index] = Some((child, at.rotation));
        }
    }
}

fn walk(
    time: Res<Time>,
    location: Res<Location>,
    settings: Res<Settings>,
    mut residents: Query<(&mut Resident, &mut Transform)>,
    mut joints: Query<&mut Transform, Without<Resident>>,
) {
    if settings.reduced_motion || time.delta_secs() == 0. {
        return;
    }
    for (mut resident, mut at) in &mut residents {
        if resident.site != *location {
            continue;
        }
        let cycle = 2. * (resident.route.travel + resident.route.pause);
        resident.elapsed = (resident.elapsed + time.delta_secs()).rem_euclid(cycle);
        let pose = resident.pose();
        *at = pose.at;
        for (joint, angle) in resident
            .joints
            .iter()
            .zip([pose.step, -pose.step, pose.arm])
        {
            if let Some((entity, rest)) = joint
                && let Ok(mut joint) = joints.get_mut(*entity)
            {
                joint.rotation = *rest * Quat::from_rotation_x(angle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::menu::Paused;

    #[test]
    fn each_walk_stops_to_work_and_turns_before_retracing_its_steps() {
        let data = GameData::load();
        for prop in &data.props {
            let Some(route) = prop.walk else { continue };
            let start = crate::props::world_pos(prop);
            let mut resident = Resident::new(prop, route, start);
            let outward = resident.span.normalize();
            let far = start + resident.span;
            for (clock, position, direction) in [
                (0., start, outward),
                (route.travel, far, outward),
                (route.travel + route.pause, far, -outward),
                (2. * route.travel + route.pause, start, -outward),
                (2. * (route.travel + route.pause), start, outward),
            ] {
                resident.elapsed = clock;
                let pose = resident.pose();
                assert!(
                    pose.at.translation.distance(position) < 0.001,
                    "{}",
                    prop.id
                );
                assert!(
                    (pose.at.rotation * Vec3::Z).dot(direction) > 0.999,
                    "{}",
                    prop.id
                );
                assert!(pose.step.abs() < 0.001, "{}", prop.id);
            }
            resident.elapsed = route.travel + route.pause * 0.5;
            let working = resident.pose();
            assert!(working.at.translation.distance(far) < 0.001);
            assert!(working.arm < -0.4);
        }
    }

    #[test]
    fn pausing_reducing_motion_and_leaving_town_hold_the_pose_and_walk_clock() {
        let data = GameData::load();
        let prop = data
            .props
            .iter()
            .find(|p| p.id == "letter_carrier")
            .unwrap();
        let mut resident = Resident::new(prop, prop.walk.unwrap(), crate::props::world_pos(prop));
        let mut app = App::new();
        app.insert_resource(State::new(Phase::Day))
            .insert_resource(Location::Market)
            .init_resource::<Settings>()
            .init_resource::<Paused>()
            .init_resource::<Time>();
        install(&mut app);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(200));
        let rest = Quat::from_rotation_z(0.1);
        let boot = app.world_mut().spawn(Transform::from_rotation(rest)).id();
        resident.joints[0] = Some((boot, rest));
        let walker = app.world_mut().spawn((resident, Transform::default())).id();
        let frame = |app: &App| {
            let world = app.world();
            (
                *world.get::<Transform>(walker).unwrap(),
                world.get::<Resident>(walker).unwrap().elapsed,
                world.get::<Transform>(boot).unwrap().rotation,
            )
        };
        app.update();
        let moving = frame(&app);
        let angle = app.world().get::<Resident>(walker).unwrap().pose().step;
        assert!(angle.abs() > 0.01);
        assert!(
            moving
                .2
                .abs_diff_eq(rest * Quat::from_rotation_x(angle), 0.0001)
        );
        app.world_mut().resource_mut::<Settings>().reduced_motion = true;
        app.update();
        assert_eq!(frame(&app), moving);
        app.world_mut().resource_mut::<Settings>().reduced_motion = false;
        app.world_mut().resource_mut::<Paused>().0 = true;
        app.update();
        assert_eq!(frame(&app), moving);
        app.world_mut().resource_mut::<Paused>().0 = false;
        app.insert_resource(Location::Shop);
        app.update();
        assert_eq!(frame(&app), moving);
        app.insert_resource(Location::Market);
        app.update();
        assert!(frame(&app).1 > moving.1);
    }
}
