//! The ends of the mooring lines stay on the bollards while the hull lifts beneath them.
use super::{ARRIVE_SECS, Boat, BoughtTonight, Builder, container_spec};
use crate::data::{GameData, asset_exists};
use crate::harbor::{HARBOR, MOORING_POSTS};
use crate::save::Settings;
use crate::shop::model_or_box;
use bevy::prelude::*;

const LINE: &str = "models/harbor/mooring_line.glb";
const FENDER: &str = "models/harbor/cork_fender.glb";
const MAKE_FAST: f32 = 0.65;
const DAMPING: f32 = 3.0;
const FREQUENCY: f32 = 7.0;

#[derive(Default)]
pub(super) struct Load {
    weight: Option<f32>,
    lift: f32,
    speed: f32,
}

impl Load {
    /// The first observation may be a saved cart. It sets the waterline without replaying a trade.
    pub(super) fn weigh(&mut self, bought: &BoughtTonight, data: &GameData) -> f32 {
        let weight = bought
            .0
            .iter()
            .filter_map(|(id, n)| {
                data.product(id)
                    .map(|p| *n as f32 * container_spec(&p.container).weight)
            })
            .sum();
        let change = self.weight.replace(weight).map_or(0.0, |old| weight - old);
        self.speed = (self.speed + change * 0.70).clamp(-1.2, 1.2);
        change
    }

    pub(super) fn new_hull(&self) -> bool {
        self.weight.is_none()
    }

    fn waterline(&self) -> f32 {
        (self.weight.unwrap_or_default() * 0.017).min(0.13)
    }

    pub(super) fn settle(&mut self, dt: f32, still: bool) -> (f32, f32) {
        if still {
            self.speed = 0.0;
            self.lift = 0.0;
        } else {
            // Exact damped spring, including a long frame or a second load before the first settles.
            let a = self.lift;
            let b = (self.speed + DAMPING * a) / FREQUENCY;
            let (s, c) = (FREQUENCY * dt).sin_cos();
            let decay = (-DAMPING * dt).exp();
            self.lift = decay * (a * c + b * s);
            self.speed = decay * FREQUENCY * (b * c - a * s) - DAMPING * self.lift;
        }
        (self.waterline() + self.lift, self.lift * 0.82)
    }
}

#[derive(Component)]
pub(super) struct Line {
    cleat: Vec3,
    post: Vec3,
}

#[derive(Component)]
pub(super) struct Fender(f32);

pub(super) fn spawn(b: &mut Builder, boat: Entity) {
    for (cleat, post) in [
        (Vec3::new(-2.70, 0.18, -0.59), MOORING_POSTS[0]),
        (Vec3::new(2.62, 0.18, -0.62), MOORING_POSTS[2]),
    ] {
        let socket = b
            .commands
            .spawn((
                Line {
                    cleat,
                    post: HARBOR + post + Vec3::Y * 0.34,
                },
                Transform::from_translation(cleat),
                Visibility::Hidden,
                ChildOf(boat),
            ))
            .id();
        let rope = model_or_box(
            &mut b.commands,
            b.asset_server,
            b.meshes,
            b.materials,
            LINE,
            Vec3::new(1.0, 0.026, 0.026),
            Color::srgb(0.69, 0.55, 0.36),
        );
        b.commands
            .entity(rope)
            .insert((Pickable::IGNORE, ChildOf(socket)));
        if !asset_exists(LINE) {
            b.commands
                .entity(rope)
                .insert(Transform::from_xyz(0.5, 0.0, 0.0));
        }
    }
    for (x, y, z) in [(-2.45, 0.17, 0.78), (-0.25, 0.13, 0.89), (2.25, 0.16, 0.82)] {
        let socket = b
            .commands
            .spawn((
                Fender(x),
                Transform::from_xyz(x, y, z).with_scale(Vec3::new(1., 0.76, 1.)),
                Visibility::default(),
                ChildOf(boat),
            ))
            .id();
        let cork = model_or_box(
            &mut b.commands,
            b.asset_server,
            b.meshes,
            b.materials,
            FENDER,
            Vec3::new(0.32, 0.50, 0.29),
            Color::srgb(0.70, 0.49, 0.28),
        );
        b.commands
            .entity(cork)
            .insert((Pickable::IGNORE, ChildOf(socket)));
        if !asset_exists(FENDER) {
            b.commands
                .entity(cork)
                .insert(Transform::from_xyz(0., -0.25, 0.));
        }
    }
}

/// +X runs from the cleat to the post, and the curve hangs towards the water on either side.
fn span(start: Vec3, end: Vec3, paid_out: f32) -> Transform {
    let delta = end - start;
    let x = delta.normalize_or(Vec3::X);
    let z = x.cross(Vec3::Y).normalize_or(Vec3::Z);
    let y = z.cross(x);
    Transform::from_translation(start)
        .with_rotation(Quat::from_mat3(&Mat3::from_cols(x, y, z)))
        .with_scale(Vec3::new(delta.length() * paid_out, paid_out, 1.0))
}

pub(super) fn follow(
    time: Res<Time>,
    settings: Res<Settings>,
    boats: Query<(&Boat, &Transform)>,
    mut lines: Query<(&Line, &ChildOf, &mut Transform, &mut Visibility), Without<Boat>>,
    mut fenders: Query<(&Fender, &ChildOf, &mut Transform), (Without<Boat>, Without<Line>)>,
) {
    for (line, parent, mut at, mut visible) in &mut lines {
        let Ok((boat, hull)) = boats.get(parent.parent()) else {
            continue;
        };
        let t = match boat.leaving {
            Some(elapsed) => 1.0 - elapsed / (MAKE_FAST * 0.6),
            None => (boat.age - ARRIVE_SECS + MAKE_FAST) / MAKE_FAST,
        }
        .clamp(0., 1.);
        visible.set_if_neq(if t > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if t > 0.0 {
            let end = hull.compute_affine().inverse().transform_point3(line.post);
            at.set_if_neq(span(line.cleat, end, t * t * (3. - 2. * t)));
        }
    }
    for (f, parent, mut at) in &mut fenders {
        let Ok((boat, _)) = boats.get(parent.parent()) else {
            continue;
        };
        let roll = if settings.reduced_motion {
            0.
        } else {
            0.055 * (time.elapsed_secs() * 1.35 + f.0).sin() + boat.load.lift * 2.4
        };
        at.rotation = Quat::from_rotation_x(-roll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_lines_reach_the_fixed_posts_from_a_rocking_hull() {
        let hull =
            Transform::from_xyz(-78.6, 0.17, 0.21).with_rotation(Quat::from_rotation_x(0.13));
        for post in [MOORING_POSTS[0], MOORING_POSTS[2]] {
            let shore = HARBOR + post;
            let cleat = Vec3::new(0., 0.2, -0.6);
            let local = hull.compute_affine().inverse().transform_point3(shore);
            let line = span(cleat, local, 1.);
            assert!(
                hull.transform_point(line.transform_point(Vec3::X))
                    .distance(shore)
                    < 1e-5
            );
            assert!((line.rotation * Vec3::Y).y > 0.0);
        }
    }
    #[test]
    fn a_saved_load_has_no_bounce_and_a_refund_reverses_the_impulse() {
        let data = GameData::load();
        let id = data.products[0].id.clone();
        let mut cart = BoughtTonight::default();
        cart.0.insert(id.clone(), 1);
        let mut load = Load::default();
        assert_eq!(load.weigh(&cart, &data), 0.);
        let rest = load.settle(0.05, false).0;
        assert!(rest > 0. && load.lift == 0.);
        cart.0.insert(id.clone(), 2);
        assert!(load.weigh(&cart, &data) > 0.);
        assert!(load.settle(0.08, false).1 > 0.);
        cart.0.clear();
        assert!(load.weigh(&cart, &data) < 0.);
        assert!(load.settle(0.14, false).1 < 0.);
        assert_eq!(load.settle(0.01, true), (0., 0.));
        assert_eq!(load.settle(0.1, false), (0., 0.));
    }
}
