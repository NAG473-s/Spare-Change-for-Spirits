//! The weather of tonight's event (`weather` in `assets/data/events.ron`),
//! visible from the evening until dawn: rain and snow running down the shop
//! window and falling over the harbour, slanting rain in a storm (the candles
//! flicker harder, see `fx.rs`), and fog drifting over the shop floor and the
//! harbour water. On rainy nights the dark shape of a ghost waiting outside
//! shows behind the wet window now and then (`rain_silhouette`). Everything
//! is drawn and moved here; there are no models.

use std::f32::consts::TAU;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use serde::Deserialize;

use crate::data::GameData;
use crate::day_cycle::{Phase, Progress};
use crate::harbor::{HARBOR, Location};
use crate::rng::Rng;
use crate::scene::{ROOM_BACK_Z, ROOM_FRONT_Z, ROOM_MAX_X, ROOM_MIN_X};

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeatherNow>()
            .add_systems(Startup, spawn_weather)
            .init_resource::<NextSilhouette>()
            .add_systems(Update, (choose_weather, fall, drift_fog, rain_silhouette).chain());
    }
}

/// What a night's event looks like outside.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    Rain,
    /// Heavy, slanting rain; the candles flicker in the draught.
    Storm,
    Snow,
    /// A snowstorm: snow blown sideways; the candles flicker as in a storm.
    Blizzard,
    Fog,
}

/// The weather showing right now (none in the morning and at the market).
#[derive(Resource, Default, PartialEq)]
pub struct WeatherNow(pub Option<Weather>);

impl WeatherNow {
    pub fn is(&self, weather: Weather) -> bool {
        self.0 == Some(weather)
    }

    /// Wind strong enough to blow the rain or snow and the candle flames.
    pub fn stormy(&self) -> bool {
        self.is(Weather::Storm) || self.is(Weather::Blizzard)
    }
}

/// The shop window (centre, half size) the drops run down, just in front of
/// the glass; the window itself is built in `scene.rs`.
const WINDOW: (Vec3, Vec3) = (
    Vec3::new(-3.2, 2.0, ROOM_BACK_Z + 0.14),
    Vec3::new(0.6, 0.62, 0.01),
);
/// The air over the harbour where rain and snow fall.
const HARBOR_AIR: (Vec3, Vec3) = (Vec3::new(0.0, 3.0, -1.0), Vec3::new(10.0, 3.0, 5.0));
/// Drops (and flakes) on the window and over the harbour.
const WINDOW_DROPS: usize = 45;
const HARBOR_DROPS: usize = 260;
/// How fast drops slide down the glass and fall outside (metres a second).
const WINDOW_SPEED: (f32, f32) = (0.35, 0.9);
const RAIN_SPEED: (f32, f32) = (5.0, 7.0);
const SNOW_SPEED: (f32, f32) = (0.3, 0.6);
/// In a storm the rain falls this much faster and is blown sideways.
const STORM_SPEED: f32 = 1.5;
const STORM_SLANT: f32 = 0.45;
/// Fog patches: how many, their size and how fast they drift.
const SHOP_FOG: usize = 7;
const HARBOR_FOG: usize = 10;
const FOG_SIZE: (f32, f32) = (2.6, 4.2);
/// How far (each way) and how fast (radians a second) fog sways.
const FOG_SWAY: f32 = 0.9;
const FOG_DRIFT: (f32, f32) = (0.05, 0.12);
const FOG_ALPHA: f32 = 0.3;
/// Fog in the shop hangs at about knee height, through the furniture.
const FOG_HEIGHT: f32 = 0.4;

/// Where a drop or flake belongs and how it falls.
#[derive(Component)]
struct Drop {
    area: (Vec3, Vec3),
    /// On the window glass (slides) or in the open air (falls).
    on_glass: bool,
    snow: bool,
    speed: f32,
    phase: f32,
}

#[derive(Component)]
struct Fog {
    location: Location,
    base: Vec3,
    speed: f32,
    phase: f32,
}

fn spawn_weather(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut rng: ResMut<Rng>,
) {
    let glow = |materials: &mut Assets<StandardMaterial>, a: f32, e: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgba(0.85, 0.9, 1.0, a),
            emissive: LinearRgba::rgb(e, e * 1.05, e * 1.15),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })
    };
    let rain = glow(&mut materials, 0.45, 0.35);
    let glass_rain = glow(&mut materials, 0.75, 0.9);
    let snow = glow(&mut materials, 0.9, 0.55);
    let glass_drop = meshes.add(Cuboid::new(0.022, 0.11, 0.006));
    let streak = meshes.add(Cuboid::new(0.014, 0.28, 0.014));
    let flake = meshes.add(Sphere::new(0.025).mesh().ico(0).unwrap());
    let harbor = (HARBOR + HARBOR_AIR.0, HARBOR_AIR.1);
    for (area, count, on_glass) in [(WINDOW, WINDOW_DROPS, true), (harbor, HARBOR_DROPS, false)] {
        for snowy in [false, true] {
            let speed = match (on_glass, snowy) {
                (true, _) => WINDOW_SPEED,
                (false, false) => RAIN_SPEED,
                (false, true) => SNOW_SPEED,
            };
            for _ in 0..count {
                let (centre, half) = area;
                let offset = Vec3::new(
                    rng.range(-half.x, half.x),
                    rng.range(-half.y, half.y),
                    rng.range(-half.z, half.z),
                );
                let mesh = match (snowy, on_glass) {
                    (true, _) => flake.clone(),
                    (false, true) => glass_drop.clone(),
                    (false, false) => streak.clone(),
                };
                commands.spawn((
                    Drop {
                        area,
                        on_glass,
                        snow: snowy,
                        speed: rng.range(speed.0, speed.1),
                        phase: rng.range(0.0, TAU),
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(match (snowy, on_glass) {
                        (true, _) => snow.clone(),
                        (false, true) => glass_rain.clone(),
                        (false, false) => rain.clone(),
                    }),
                    Transform::from_translation(centre + offset),
                    Visibility::Hidden,
                    NotShadowCaster,
                    Pickable::IGNORE,
                ));
            }
        }
    }
    // Fog: soft white patches lying just above the floor and the water.
    let puff = images.add(crate::harbor::soft_puff(64));
    let fog = materials.add(StandardMaterial {
        base_color: Color::srgba(0.8, 0.85, 0.95, FOG_ALPHA),
        base_color_texture: Some(puff),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let shop = (
        Vec3::new(
            (ROOM_MIN_X + ROOM_MAX_X) / 2.0,
            FOG_HEIGHT,
            (ROOM_BACK_Z + ROOM_FRONT_Z) / 2.0,
        ),
        // Kept inside the walls, also when a patch sways.
        Vec3::new(
            (ROOM_MAX_X - ROOM_MIN_X) / 2.0 - FOG_SIZE.1 / 2.0 - FOG_SWAY,
            0.15,
            (ROOM_FRONT_Z - ROOM_BACK_Z) / 2.0 - FOG_SIZE.1 * 0.3,
        ),
    );
    let water = (
        HARBOR + Vec3::new(0.0, 0.35, 1.0),
        Vec3::new(11.0, 0.1, 4.0),
    );
    for (location, area, count) in [
        (Location::Shop, shop, SHOP_FOG),
        (Location::Harbor, water, HARBOR_FOG),
    ] {
        for _ in 0..count {
            let (centre, half) = area;
            let size = rng.range(FOG_SIZE.0, FOG_SIZE.1);
            let at = centre
                + Vec3::new(
                    rng.range(-half.x, half.x),
                    rng.range(-half.y, half.y),
                    rng.range(-half.z, half.z),
                );
            commands.spawn((
                Fog {
                    location,
                    base: at,
                    speed: rng.range(FOG_DRIFT.0, FOG_DRIFT.1),
                    phase: rng.range(0.0, TAU),
                },
                Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(size, size * 0.6) / 2.0))),
                MeshMaterial3d(fog.clone()),
                Transform::from_translation(at),
                Visibility::Hidden,
                NotShadowCaster,
                Pickable::IGNORE,
            ));
        }
    }
}

/// Tonight's weather from the evening until dawn.
fn choose_weather(
    data: Res<GameData>,
    progress: Res<Progress>,
    phase: Res<State<Phase>>,
    mut now: ResMut<WeatherNow>,
) {
    let wanted = WeatherNow(
        matches!(phase.get(), Phase::Evening | Phase::Night)
            .then(|| data.event(progress.calendar(&data)))
            .flatten()
            .and_then(|e| e.weather),
    );
    now.set_if_neq(wanted);
}

fn show(mut visibility: Mut<Visibility>, on: bool) {
    visibility.set_if_neq(if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}

/// Drops slide down the glass with a little wobble, rain and snow fall
/// outside; each starts again at the top.
fn fall(
    time: Res<Time>,
    now: Res<WeatherNow>,
    location: Res<Location>,
    mut drops: Query<(&Drop, &mut Transform, &mut Visibility)>,
) {
    let storm = now.stormy();
    let raining = now.is(Weather::Rain) || now.is(Weather::Storm);
    let snowing = now.is(Weather::Snow) || now.is(Weather::Blizzard);
    let (dt, t) = (time.delta_secs(), time.elapsed_secs());
    let changed = now.is_changed() || location.is_changed();
    if !changed && ((!raining && !snowing) || dt == 0.0) {
        return;
    }
    for (d, mut transform, visibility) in &mut drops {
        let here = if d.on_glass {
            Location::Shop
        } else {
            Location::Harbor
        };
        let on = here == *location && if d.snow { snowing } else { raining };
        if changed {
            show(visibility, on);
        }
        if !on {
            continue;
        }
        let (centre, half) = d.area;
        let speed = if storm {
            d.speed * STORM_SPEED
        } else {
            d.speed
        };
        let p = &mut transform.translation;
        p.y -= speed * dt;
        if d.on_glass && !d.snow {
            // Drops on glass pause and run on.
            p.y += speed * dt * 0.6 * (t * 1.3 + d.phase).sin().max(0.0);
        } else if storm && !d.on_glass {
            p.x += speed * STORM_SLANT * dt;
        } else if d.snow {
            p.x += 0.12 * (t * 0.8 + d.phase).sin() * dt;
        }
        if p.x > centre.x + half.x {
            p.x -= 2.0 * half.x;
        } else if p.x < centre.x - half.x {
            p.x += 2.0 * half.x;
        }
        if p.y < centre.y - half.y {
            p.y += 2.0 * half.y;
        }
        transform.rotation = if storm && !d.on_glass && !d.snow {
            Quat::from_rotation_z(STORM_SLANT.atan())
        } else {
            Quat::IDENTITY
        };
    }
}

/// Fog patches sway slowly to and fro.
fn drift_fog(
    time: Res<Time>,
    now: Res<WeatherNow>,
    location: Res<Location>,
    mut patches: Query<(&Fog, &mut Transform, &mut Visibility)>,
) {
    let foggy = now.is(Weather::Fog);
    let changed = now.is_changed() || location.is_changed();
    if !changed && (!foggy || time.delta().is_zero()) {
        return;
    }
    let t = time.elapsed_secs();
    for (f, mut transform, visibility) in &mut patches {
        let on = foggy && *location == f.location;
        if changed {
            show(visibility, on);
        }
        if !on {
            continue;
        }
        let a = t * f.speed + f.phase;
        transform.translation =
            f.base + Vec3::new(FOG_SWAY * a.sin(), 0.0, 0.25 * FOG_SWAY * (a * 1.7).cos());
        transform.rotation = Quat::from_rotation_y(0.15 * (t * 0.1 + f.phase).sin());
    }
}

/// Where the silhouette stands: just behind the running drops, on the glass.
const SILHOUETTE_AT: Vec3 = Vec3::new(-3.2, 2.05, ROOM_BACK_Z + 0.2);
const SILHOUETTE_SIZE: Vec2 = Vec2::new(0.75, 1.1);
/// The silhouette's colour: night-blue, nearly black.
const SILHOUETTE_TINT: Color = Color::srgb(0.04, 0.05, 0.09);

/// A ghost's shape behind the rainy window, while it lasts (seconds shown).
#[derive(Component)]
struct Silhouette(f32);

/// Seconds until the next silhouette may show.
#[derive(Resource, Default)]
struct NextSilhouette(f32);

/// On a rainy or stormy night, now and then a ghost waiting outside shows
/// as a dark shape behind the wet glass, then fades again. Only atmosphere:
/// it tells nothing the player needs.
#[allow(clippy::too_many_arguments)]
fn rain_silhouette(
    mut commands: Commands,
    time: Res<Time>,
    now: Res<WeatherNow>,
    phase: Res<State<Phase>>,
    location: Res<Location>,
    data: Res<GameData>,
    progress: Res<Progress>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextSilhouette>,
    mut shown: Query<(Entity, &mut Silhouette, &MeshMaterial3d<StandardMaterial>)>,
) {
    let c = data.config.rain_silhouette;
    let dt = time.delta_secs();
    let rainy = (now.is(Weather::Rain) || now.is(Weather::Storm))
        && *phase.get() == Phase::Night
        && *location == Location::Shop;
    for (entity, mut s, material) in &mut shown {
        s.0 += dt;
        if s.0 >= c.secs || !rainy {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut m) = materials.get_mut(&material.0) {
            let k = (s.0 / c.secs * std::f32::consts::PI).sin();
            m.base_color = SILHOUETTE_TINT.with_alpha(c.alpha * k);
        }
    }
    if !rainy {
        next.0 = rng.range(c.every.0, c.every.1);
        return;
    }
    next.0 -= dt;
    if next.0 > 0.0 || !shown.is_empty() {
        return;
    }
    next.0 = rng.range(c.every.0, c.every.1);
    // One of the ghosts that may come tonight, whose picture exists.
    let night = progress.calendar(&data);
    let ghosts: Vec<&str> = data
        .ghosts
        .iter()
        .filter(|g| g.spawn_weight > 0.0 && g.unlock_night <= night)
        .map(|g| g.id.as_str())
        .filter(|id| crate::data::asset_exists(&format!("sprites/ghosts/{id}.png")))
        .collect();
    if ghosts.is_empty() {
        return;
    }
    let id = ghosts[rng.index(ghosts.len())];
    commands.spawn((
        Silhouette(0.0),
        Mesh3d(meshes.add(Rectangle::new(SILHOUETTE_SIZE.x, SILHOUETTE_SIZE.y))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: SILHOUETTE_TINT.with_alpha(0.0),
            base_color_texture: Some(asset_server.load(format!("sprites/ghosts/{id}.png"))),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        // A little off centre, as if standing outside by the glass.
        Transform::from_translation(SILHOUETTE_AT + Vec3::X * rng.range(-0.25, 0.25)),
        NotShadowCaster,
        Pickable::IGNORE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rain_silhouette_is_rare_short_and_never_opaque() {
        let c = GameData::load().config.rain_silhouette;
        assert!(c.every.0 > c.secs && c.every.0 <= c.every.1, "{c:?}");
        assert!(c.alpha > 0.0 && c.alpha < 1.0);
    }

    #[test]
    fn weather_comes_with_its_events() {
        let data = GameData::load();
        let weather = |id: &str| data.events.iter().find(|e| e.id == id).unwrap().weather;
        assert_eq!(weather("fog"), Some(Weather::Fog));
        assert_eq!(weather("storm"), Some(Weather::Storm));
        assert_eq!(weather("night_showers"), Some(Weather::Rain));
        assert_eq!(weather("midwinter"), Some(Weather::Snow));
        assert_eq!(weather("harvest_moon"), None);
    }
}
