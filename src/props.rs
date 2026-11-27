//! Shop fittings, houses and neighbours from `assets/data/props.ron`.
//! Each loads its own model or a primitive stand-in at the same position.

pub(crate) mod mail;
mod residents;
mod rounds;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::data::{GameData, PropDef, Scene, Shape, asset_exists, rgb};
use crate::fx::Flicker;
use crate::harbor::HARBOR;

pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_props);
        residents::install(app);
        rounds::install(app);
        mail::install(app);
    }
}

/// Where a prop's model lives.
pub fn prop_model_path(id: &str) -> String {
    format!("models/props/{id}.glb")
}

/// World position of a prop: the harbour is built away from the shop.
fn world_pos(prop: &PropDef) -> Vec3 {
    let pos = Vec3::new(prop.pos.0, prop.pos.1, prop.pos.2);
    match prop.scene {
        Scene::Shop => pos,
        Scene::Harbor => HARBOR + pos,
        Scene::Market => crate::market::MARKET + pos,
    }
}

fn spawn_props(
    mut commands: Commands,
    data: Res<GameData>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (i, prop) in data.props.iter().enumerate() {
        let path = prop_model_path(&prop.id);
        let size = Vec3::new(prop.size.0, prop.size.1, prop.size.2);
        let position = world_pos(prop);
        let root = commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(Quat::from_rotation_y(prop.turn.to_radians())),
                Visibility::default(),
                Pickable::IGNORE,
            ))
            .id();
        if prop.id == "shop_post_gate" {
            commands.entity(root).insert(mail::PostGate);
        }
        let mut resident = prop
            .walk
            .map(|walk| residents::Resident::new(prop, walk, position));
        let has_model = asset_exists(&path);
        if has_model {
            let mut model = commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))),
                Transform::default(),
                ChildOf(root),
            ));
            model.observe(crate::scene::ignore_scene_picking);
            if matches!(prop.id.as_str(), "dyer_house" | "fishing_net") {
                model.observe(rounds::bind_fabric);
            }
            if prop.walk.is_some() {
                model.observe(residents::bind_joints);
            }
        } else if let Some(resident) = &mut resident {
            resident.stand_in(
                &mut commands,
                &mut meshes,
                &mut materials,
                root,
                rgb(prop.color),
            );
        } else if prop.id == "fishing_net" {
            rounds::fallback_fabric(&mut commands, &mut meshes, &mut materials, root, false);
        } else {
            if prop.id == "dyer_house" {
                rounds::fallback_fabric(&mut commands, &mut meshes, &mut materials, root, true);
            }
            // Keep the plate in front of a replacement gate and the loaves on top of an empty counter.
            let size = match prop.id.as_str() {
                "shop_post_gate" => Vec3::new(size.x, size.y, 0.18),
                "bread_stall" => Vec3::new(size.x, 0.752, size.z),
                _ => size,
            };
            let mut body = size;
            let mesh = match prop.shape {
                Shape::Box => meshes.add(Cuboid::from_size(size)),
                Shape::Cylinder => meshes.add(Cylinder::new(size.x.max(size.z) / 2.0, size.y)),
                Shape::House => {
                    body *= Vec3::new(0.91, 0.61, 0.91);
                    let gable = Triangle2d::new(
                        Vec2::new(-size.x * 0.5, 0.),
                        Vec2::new(size.x * 0.5, 0.),
                        Vec2::new(0., size.y - body.y),
                    );
                    commands.spawn((
                        Mesh3d(meshes.add(Extrusion::new(gable, size.z))),
                        MeshMaterial3d(materials.add(Color::srgb(0.32, 0.38, 0.40))),
                        Transform::from_xyz(0., body.y, 0.),
                        Pickable::IGNORE,
                        ChildOf(root),
                    ));
                    meshes.add(Cuboid::from_size(body))
                }
            };
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: rgb(prop.color),
                    perceptual_roughness: 0.8,
                    ..default()
                })),
                Transform::from_xyz(0.0, body.y / 2.0, 0.0),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        if let Some(resident) = resident {
            commands.entity(root).insert((resident.pose().at, resident));
        }
        if let Some((height, strength)) = prop.light {
            let light = commands
                .spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.72, 0.42),
                        intensity: strength,
                        range: 4.0,
                        radius: 0.03,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Flicker {
                        base: strength,
                        phase: i as f32 * 2.3,
                    },
                    Transform::from_xyz(0.0, height, 0.0),
                    ChildOf(root),
                ))
                .id();
            // A placeholder needs a visible flame; a model has its own.
            if !has_model {
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.03))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        emissive: LinearRgba::rgb(6.0, 3.4, 1.2),
                        ..default()
                    })),
                    NotShadowCaster,
                    Pickable::IGNORE,
                    ChildOf(light),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn props_sit_inside_their_scene() {
        let data = GameData::load();
        assert!(!data.props.is_empty());
        for p in &data.props {
            let (w, h, d) = p.size;
            assert!(w > 0.0 && h > 0.0 && d > 0.0, "{} has no size", p.id);
            // Both dioramas span about x -7.5..8 and z -9..8.
            assert!(
                (-8.0..=8.0).contains(&p.pos.0) && (-9.5..=8.0).contains(&p.pos.2),
                "{} stands outside its scene: {:?}",
                p.id,
                p.pos
            );
        }
    }
}
