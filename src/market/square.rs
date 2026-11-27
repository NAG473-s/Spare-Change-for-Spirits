//! The button set into the cart lane leads up to the merchants' gallery.
use super::Kit;
use crate::save::Settings;
use crate::scene::village::Breeze;
use bevy::prelude::*;

const BUTTON: Vec3 = Vec3::new(1.97, 0.004, -1.30);
const WALK: Vec3 = Vec3::new(0.50, 0., -5.34);
const BENCH: Vec3 = Vec3::new(-1.55, 0., 5.92);
const BENCH_ANGLE: f32 = 2.94;
/// Both ends of the street and the return along the eastern cart lane.
const BORDERS: [(f32, f32, f32); 3] = [
    (-7.72, -1.98, 1.36),
    (7.66, -2.16, -1.36),
    (9.21, 1.82, -0.29),
];
/// All five gables start at the upper-storey sill of the original village house.
const GABLE_SILL: Vec3 = Vec3::new(0., 1.27, 0.22);
/// Foot of the rotating fish, on the copper spindle above the sail loft.
const VANE_SOCKET: Vec3 = Vec3::new(0., 1.88, -0.04);
const STONE: Color = Color::srgb(0.63, 0.55, 0.44);
const TIMBER: Color = Color::srgb(0.34, 0.22, 0.17);
const VERDIGRIS: Color = Color::srgb(0.29, 0.51, 0.48);

#[derive(Component)]
pub(super) struct WeatherVane;

pub(super) fn spawn(k: &mut Kit, root: Entity) {
    if !k.model(
        root,
        "models/market/button_paving.glb",
        Transform::from_translation(BUTTON),
    ) {
        k.part(
            root,
            Cylinder::new(1.51, 0.055),
            STONE,
            Transform::from_translation(BUTTON + Vec3::Y * 0.0275),
        );
    }
    if !k.model(
        root,
        "models/market/shopfront_walk.glb",
        Transform::from_translation(WALK),
    ) {
        k.part(
            root,
            Cuboid::new(13.6, 0.14, 1.05),
            STONE,
            Transform::from_translation(WALK + Vec3::Y * 0.07),
        );
    }
    let bench = k.node(
        Some(root),
        Transform::from_translation(BENCH).with_rotation(Quat::from_rotation_y(BENCH_ANGLE)),
    );
    if !k.model(
        bench,
        "models/market/square_bench.glb",
        Transform::default(),
    ) {
        k.part(
            bench,
            Cuboid::new(1.72, 0.10, 0.48),
            VERDIGRIS,
            Transform::from_xyz(0., 0.43, 0.),
        );
        k.part(
            bench,
            Cuboid::new(1.72, 0.27, 0.07),
            VERDIGRIS,
            Transform::from_xyz(0., 0.74, -0.20),
        );
        for x in [-0.62, 0.62] {
            k.part(
                bench,
                Cuboid::new(0.09, 0.40, 0.42),
                TIMBER,
                Transform::from_xyz(x, 0.20, 0.),
            );
        }
    }

    for (x, z, angle) in BORDERS {
        let bed = k.node(
            Some(root),
            Transform::from_xyz(x, 0., z).with_rotation(Quat::from_rotation_y(angle)),
        );
        if !k.model(bed, "models/market/herb_border.glb", Transform::default()) {
            k.part(
                bed,
                Cuboid::new(3.82, 0.40, 0.64),
                STONE,
                Transform::from_xyz(0., 0.20, 0.),
            );
            k.part(
                bed,
                Cuboid::new(3.36, 0.24, 0.40),
                Color::srgb(0.43, 0.50, 0.40),
                Transform::from_xyz(0., 0.50, 0.),
            );
        }
    }
    houses(k, root);
}

fn houses(k: &mut Kit, root: Entity) {
    for (index, (gable, x, z, w, h)) in [
        ("sail_loft", -5.0, -6.0, 2.2, 2.2),
        ("cloth_hall", -2.2, -6.6, 1.8, 2.6),
        ("bakehouse", 0.6, -6.2, 2.4, 2.0),
        ("bell_lodge", 3.4, -6.7, 2.0, 2.5),
        ("stationer", 6.0, -6.1, 2.2, 2.1),
    ]
    .into_iter()
    .enumerate()
    {
        let house = k.node(
            Some(root),
            Transform::from_xyz(x, 0., z).with_scale(Vec3::new(w / 2.2, h / 2.4, 1.)),
        );
        let variant = format!("models/harbor/village_house_{}.glb", index % 4 + 1);
        if !k.model(house, &variant, Transform::default())
            && !k.model(
                house,
                "models/harbor/village_house.glb",
                Transform::default(),
            )
        {
            k.part(
                house,
                Cuboid::new(2.2, 1.50, 1.8),
                Color::srgb(0.55, 0.5, 0.48),
                Transform::from_xyz(0., 0.75, 0.),
            );
            for side in [-1.0, 1.0] {
                k.part(
                    house,
                    Cuboid::new(1.4, 0.1, 2.),
                    TIMBER,
                    Transform::from_xyz(side * 0.52, 1.83, 0.)
                        .with_rotation(Quat::from_rotation_z(-side * 0.6)),
                );
            }
        }
        let upper = k.node(Some(house), Transform::from_translation(GABLE_SILL));
        if !k.model(
            upper,
            &format!("models/market/gable_{gable}.glb"),
            Transform::default(),
        ) {
            k.part(
                upper,
                Cuboid::new(1.24, 0.82, 0.88),
                TIMBER,
                Transform::from_xyz(0., 0.41, 0.),
            );
            for side in [-1.0, 1.0] {
                k.part(
                    upper,
                    Cuboid::new(0.91, 0.09, 1.),
                    VERDIGRIS,
                    Transform::from_xyz(side * 0.32, 1.09, 0.)
                        .with_rotation(Quat::from_rotation_z(-side * 0.7)),
                );
            }
            if gable == "sail_loft" {
                let mast_height = 0.60;
                k.part(
                    upper,
                    Cylinder::new(0.032, mast_height),
                    TIMBER,
                    Transform::from_translation(VANE_SOCKET - Vec3::Y * mast_height / 2.),
                );
            }
        }
        if gable == "sail_loft" {
            let vane = k.node(Some(upper), Transform::from_translation(VANE_SOCKET));
            k.commands.entity(vane).insert(WeatherVane);
            if !k.model(vane, "models/market/fish_vane.glb", Transform::default()) {
                k.part(
                    vane,
                    Cuboid::new(0.88, 0.16, 0.045),
                    VERDIGRIS,
                    Transform::from_xyz(0., 0.09, 0.),
                );
            }
        }
    }
}

pub(super) fn turn_vane(
    wind: Res<Breeze>,
    settings: Res<Settings>,
    mut vanes: Query<&mut Transform, With<WeatherVane>>,
) {
    let angle = if settings.reduced_motion {
        0.
    } else {
        (wind.gust(-5.) - 0.5) * 1.45
    };
    let rotation = Quat::from_rotation_y(angle);
    for mut vane in &mut vanes {
        if vane.rotation != rotation {
            vane.rotation = rotation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::market::{FigureKind, STALL_HALF_FOOTPRINT, stall_transform};

    #[test]
    fn permanent_and_seasonal_pitches_leave_a_walkable_ring_around_the_button() {
        let paving_radius = 1.60;
        let clearance = 0.30;
        for vendor in &GameData::load().vendors {
            let pitch = stall_transform(vendor);
            let centre = pitch
                .compute_affine()
                .inverse()
                .transform_point3(BUTTON)
                .xz();
            let outside = (centre.abs() - STALL_HALF_FOOTPRINT).max(Vec2::ZERO);
            let trader = FigureKind::for_vendor(&vendor.id).spot().xz();
            assert!(
                outside.length() >= paving_radius + clearance,
                "{} awning",
                vendor.id
            );
            assert!(
                centre.distance(trader) - 0.35 >= paving_radius + clearance,
                "{} trader",
                vendor.id
            );
        }
    }
}
