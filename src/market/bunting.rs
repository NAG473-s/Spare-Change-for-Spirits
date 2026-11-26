//! Dyed swallowtails hang from the hemp line behind the traders.
use super::Kit;
use crate::data::{GameData, asset_exists, rgb};
use crate::rng::Rng;
use crate::save::Settings;
use crate::scene::village::Breeze;
use bevy::prelude::*;

const LINE: &str = "models/market/bunting_line.glb";
const FLAG: &str = "sprites/market/fair_pennant.png";
const COUNT: usize = 30;
const SPAN: f32 = 13.8;
const ORIGIN: Vec3 = Vec3::new(0.10, 0., -3.6);
const DROP: f32 = 0.34;

#[derive(Component)]
pub(super) struct Flag {
    along: f32,
    phase: f32,
}

fn knot(t: f32) -> Vec3 {
    Vec3::new(SPAN * (t - 0.5), 3.10 - 2.20 * t * (1. - t), 0.)
}

pub(super) fn spawn(k: &mut Kit, market: Entity, data: &GameData, rng: &mut Rng) {
    let line = k.node(Some(market), Transform::from_translation(ORIGIN));
    if !k.model(line, LINE, Transform::default()) {
        let wood = Color::srgb(0.48, 0.38, 0.28);
        for t in [0., 1.] {
            let top = knot(t);
            k.part(
                line,
                Cylinder::new(0.075, top.y),
                wood,
                Transform::from_xyz(top.x, top.y / 2., 0.),
            );
        }
        let cord = k.materials.add(Color::srgb(0.64, 0.52, 0.38));
        let link = k.meshes.add(Cylinder::new(0.019, 1.0));
        for i in 0..24 {
            let start = knot(i as f32 / 24.);
            let end = knot((i + 1) as f32 / 24.);
            let span = end - start;
            k.commands.spawn((
                Mesh3d(link.clone()),
                MeshMaterial3d(cord.clone()),
                Transform::from_translation((start + end) * 0.5)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, span.normalize()))
                    .with_scale(Vec3::new(1., span.length(), 1.)),
                Pickable::IGNORE,
                ChildOf(line),
            ));
        }
    }

    let painted = asset_exists(FLAG);
    let mesh = if painted {
        k.meshes.add(
            Rectangle::new(0.33, DROP)
                .mesh()
                .build()
                .translated_by(Vec3::NEG_Y * (DROP / 2.)),
        )
    } else {
        k.meshes.add(Triangle3d::new(
            Vec3::new(-0.165, 0., 0.),
            Vec3::new(0., -DROP, 0.),
            Vec3::new(0.165, 0., 0.),
        ))
    };
    let pattern = painted.then(|| k.asset_server.load(FLAG));
    let mut dyes = Vec::new();
    let mut palette: Vec<_> = data.vendors.iter().map(|v| rgb(v.color)).collect();
    if palette.is_empty() {
        palette.push(Color::WHITE);
    }
    for tint in palette {
        dyes.push(k.materials.add(StandardMaterial {
            base_color: tint.mix(&Color::srgb(0.94, 0.81, 0.60), 0.19),
            base_color_texture: pattern.clone(),
            cull_mode: None,
            double_sided: true,
            alpha_mode: if painted {
                AlphaMode::Mask(0.40)
            } else {
                AlphaMode::Opaque
            },
            perceptual_roughness: 1.,
            ..default()
        }));
    }
    for i in 0..COUNT {
        let t = (i as f32 + 0.5) / COUNT as f32;
        let at = knot(t);
        k.commands.spawn((
            Flag {
                along: at.x,
                phase: i as f32 * 0.7 + rng.range(0., 1.),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(dyes[i % dyes.len()].clone()),
            Transform::from_translation(at),
            Pickable::IGNORE,
            ChildOf(line),
        ));
    }
}

pub(super) fn sway(
    wind: Res<Breeze>,
    settings: Res<Settings>,
    mut flags: Query<(&Flag, &mut Transform)>,
) {
    for (flag, mut at) in &mut flags {
        let rotation = if settings.reduced_motion {
            Quat::IDENTITY
        } else {
            let gust = wind.gust(flag.along);
            Quat::from_rotation_x(-0.10 - 0.52 * gust - 0.08 * wind.flutter(flag.phase))
                * Quat::from_rotation_z(0.12 * gust)
        };
        if at.rotation != rotation {
            at.rotation = rotation;
        }
    }
}
