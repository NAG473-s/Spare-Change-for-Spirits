//! The hinged entrance opens onto the night quay when a customer crosses the threshold.
use bevy::prelude::*;

use super::{ShopLayout, model_or_box};
use crate::data::asset_exists;
use crate::day_cycle::Phase;
use crate::ghosts::{Ghost, GhostState};
use crate::harbor::{ShopDoor, on_shop_door_click};
use crate::save::Settings;
use crate::scene::floor;

const FRAME: Vec3 = Vec3::new(1.1, 2.3, 0.2);
pub(crate) const PIVOT: Vec3 = Vec3::new(0.438, 0.044, 0.044);
const LEAF: Vec3 = Vec3::new(0.90, 2.024, 0.15);
const BELL_MOUNT: Vec3 = Vec3::new(0.0, 2.192, 0.051);
const BELL_SCALE: f32 = 1.42;
const OPEN_ANGLE: f32 = 1.42;
const OPEN_TIME: f32 = 0.40;
const CLOSE_TIME: f32 = 0.72;
const LINGER: f32 = 0.38;
const REACH: f32 = 1.18;
const BELL_DECAY: f32 = 3.8;
const BELL_RATE: f32 = 14.0;
const BELL_ANGLE: f32 = 0.50;

#[derive(Component, Default)]
pub(crate) struct DoorSwing {
    progress: f32,
    hold: f32,
    bell_age: Option<f32>,
    occupied: bool,
}

#[derive(Component)]
pub(crate) struct ShopBell;

impl DoorSwing {
    pub(crate) fn aperture(&self) -> f32 {
        let p = self.progress;
        p * p * (3.0 - 2.0 * p)
    }

    fn advance(&mut self, occupied: bool, dt: f32, still: bool) -> (f32, f32) {
        if occupied && !self.occupied && self.progress < 0.65 {
            self.bell_age = Some(0.0);
        }
        self.occupied = occupied;
        let held = if occupied { dt } else { self.hold.min(dt) };
        self.hold = if occupied {
            LINGER
        } else {
            (self.hold - dt).max(0.0)
        };
        if still {
            self.progress = if self.hold > 0.0 { 1.0 } else { 0.0 };
            self.bell_age = None;
        } else {
            self.progress = (self.progress + held / OPEN_TIME).min(1.0);
            self.progress = (self.progress - (dt - held) / CLOSE_TIME).max(0.0);
        }
        let mut swing = 0.0;
        if let Some(age) = &mut self.bell_age {
            *age += dt;
            let envelope = (-BELL_DECAY * *age).exp();
            if envelope < 0.005 {
                self.bell_age = None;
            } else {
                swing = BELL_ANGLE * envelope * (BELL_RATE * *age).sin();
            }
        }
        (self.aperture(), swing)
    }
}

pub(super) fn spawn(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    layout: &ShopLayout,
) {
    let entrance = commands
        .spawn((
            ShopDoor,
            Transform::from_translation(floor(layout.door_pos(), 0.0))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
        ))
        .observe(on_shop_door_click)
        .id();
    let frame_path = "models/shop/door.glb";
    let frame = model_or_box(
        commands,
        assets,
        meshes,
        materials,
        frame_path,
        Vec3::new(FRAME.x, FRAME.y, 0.035),
        Color::srgb(0.18, 0.25, 0.33),
    );
    commands.entity(frame).insert(ChildOf(entrance));
    if !asset_exists(frame_path) {
        commands
            .entity(frame)
            .insert(Transform::from_xyz(0.0, FRAME.y / 2.0, -0.09));
    }
    let hinge = commands
        .spawn((
            DoorSwing::default(),
            Transform::from_translation(PIVOT),
            Visibility::default(),
            ChildOf(entrance),
        ))
        .id();
    let path = "models/shop/door_leaf.glb";
    let leaf = model_or_box(
        commands,
        assets,
        meshes,
        materials,
        path,
        LEAF,
        Color::srgb(0.39, 0.26, 0.20),
    );
    commands.entity(leaf).insert(ChildOf(hinge));
    if !asset_exists(path) {
        commands
            .entity(leaf)
            .insert(Transform::from_xyz(-LEAF.x / 2.0, LEAF.y / 2.0, 0.0));
    }
    let mount = commands
        .spawn((
            ShopBell,
            Transform::from_translation(BELL_MOUNT).with_scale(Vec3::splat(BELL_SCALE)),
            Visibility::default(),
            ChildOf(entrance),
        ))
        .id();
    let path = "models/shop/door_bell.glb";
    let bell = model_or_box(
        commands,
        assets,
        meshes,
        materials,
        path,
        Vec3::new(0.11, 0.105, 0.11),
        Color::srgb(0.78, 0.64, 0.36),
    );
    commands.entity(bell).insert(ChildOf(mount));
    if !asset_exists(path) {
        commands
            .entity(bell)
            .insert(Transform::from_xyz(0.0, -0.0525, 0.0));
    }
}

pub(crate) fn animate(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    settings: Res<Settings>,
    layout: Res<ShopLayout>,
    guests: Query<&Ghost>,
    mut door: Single<(&mut DoorSwing, &mut Transform)>,
    mut bell: Single<&mut Transform, (With<ShopBell>, Without<DoorSwing>)>,
) {
    let (motion, leaf) = &mut *door;
    let (opening, swing) = if *phase.get() == Phase::Night {
        let threshold = layout.door_pos();
        let occupied = guests.iter().any(|g| {
            matches!(
                g.state,
                GhostState::Entering | GhostState::Fleeing | GhostState::Leaving
            ) && g.pos.distance_squared(threshold) <= REACH * REACH
        });
        if !occupied && motion.progress == 0.0 && motion.bell_age.is_none() {
            return;
        }
        motion.advance(occupied, time.delta_secs(), settings.reduced_motion)
    } else {
        if motion.progress == 0.0 && !motion.occupied && motion.bell_age.is_none() {
            return;
        }
        **motion = DoorSwing::default();
        (0.0, 0.0)
    };
    leaf.rotation = Quat::from_rotation_y(OPEN_ANGLE * opening);
    bell.rotation = Quat::from_rotation_z(swing);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queue_crossing_holds_the_leaf_and_reduced_motion_stops_the_bell() {
        let mut door = DoorSwing::default();
        let (partly_open, bell) = door.advance(true, 0.12, false);
        assert!(partly_open > 0.0 && partly_open < 1.0 && bell.abs() > 0.01);
        assert_eq!(door.advance(true, 0.5, false).0, 1.0);
        assert_eq!(door.advance(false, LINGER / 2.0, false).0, 1.0);
        assert_eq!(door.advance(true, 0.01, false).0, 1.0);
        let closing = door.advance(false, LINGER + 0.08, false).0;
        let p = 1.0 - 0.08 / CLOSE_TIME;
        assert!((closing - p * p * (3.0 - 2.0 * p)).abs() < 1e-5);
        assert_eq!(door.advance(true, 0.01, true), (1.0, 0.0));
        assert_eq!(door.advance(false, LINGER + 0.01, true), (0.0, 0.0));
        assert_eq!(door.advance(false, 0.1, false), (0.0, 0.0));
    }
}
