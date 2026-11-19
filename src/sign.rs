//! The "closed / open" sign hanging in the shop window. In the evening a
//! click turns it round (the "open the shop" button does the same): the sign
//! flips, the wall clock runs on to midnight and strikes, and the night
//! begins. At dawn the sign turns back to "closed".

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::data::asset_exists;
use crate::day_cycle::Phase;
use crate::fx::Cue;
use crate::harbor::Location;
use crate::i18n::Lang;
use crate::scene::{ClockOverride, ROOM_BACK_Z, shop_minutes, world_label};
use crate::ui::{Action, UiFont, text_font};

pub struct SignPlugin;

impl Plugin for SignPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Opening>()
            .init_resource::<SignHovered>()
            .add_systems(Startup, spawn_sign)
            .add_systems(
                OnEnter(Phase::Night),
                |mut c: ResMut<ClockOverride>, mut o: ResMut<Opening>| {
                    c.0 = None;
                    o.0 = None;
                },
            )
            .add_systems(
                Update,
                (
                    start_opening.run_if(in_state(Phase::Evening)),
                    run_opening.run_if(in_state(Phase::Evening)),
                    turn_sign,
                    update_sign_label,
                ),
            );
    }
}

/// Seconds since the shop is being opened (sign turned, clock running).
#[derive(Resource, Default)]
pub struct Opening(pub Option<f32>);

#[derive(Resource, Default)]
struct SignHovered(bool);

/// The board on its two cords; turns about the vertical axis.
#[derive(Component)]
struct Sign {
    /// Current turn, 0 = "closed" side to the front, 1 = "open".
    turn: f32,
}

#[derive(Component)]
struct SignLabel;

/// Hangs inside the arched window of the back wall.
const SIGN_POS: Vec3 = Vec3::new(-3.2, 1.45, ROOM_BACK_Z + 0.2);
const SIGN_MODEL: &str = "models/shop/shop_sign.glb";
const BOARD: Vec3 = Vec3::new(0.98, 0.44, 0.03);
/// The sign flips during the first part of the opening ...
const FLIP_SECS: f32 = 0.6;
/// ... then the clock runs on to midnight ...
const CLOCK_FROM: f32 = 0.4;
const CLOCK_SECS: f32 = 1.6;
/// ... strikes, and the night begins a moment later.
const OPEN_AT: f32 = CLOCK_FROM + CLOCK_SECS + 1.2;

fn spawn_sign(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sign = commands
        .spawn((
            Sign { turn: 0.0 },
            Transform::from_translation(SIGN_POS),
            Visibility::default(),
        ))
        .observe(on_sign_click)
        .observe(|_: On<PointerOver>, mut hovered: ResMut<SignHovered>| hovered.0 = true)
        .observe(|_: On<PointerOut>, mut hovered: ResMut<SignHovered>| hovered.0 = false)
        .id();
    // A model (`models/shop/shop_sign.glb`: "closed" side to the front,
    // "open" on the back, no lettering) or a painted board on two cords.
    if asset_exists(SIGN_MODEL) {
        commands.spawn((
            WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(SIGN_MODEL))),
            Transform::default(),
            ChildOf(sign),
        ));
    } else {
        let board = meshes.add(Cuboid::from_size(BOARD));
        let frame = meshes.add(Cuboid::new(BOARD.x + 0.05, BOARD.y + 0.05, BOARD.z * 0.8));
        // A faint glow keeps the painted board readable against the dark window.
        let mut paint = |color: Color, glow: f32| {
            materials.add(StandardMaterial {
                base_color: color,
                emissive: color.to_linear() * glow,
                perceptual_roughness: 0.8,
                ..default()
            })
        };
        let wood = paint(Color::srgb(0.42, 0.29, 0.18), 0.1);
        let closed = paint(Color::srgb(0.62, 0.18, 0.15), 0.35);
        let open = paint(Color::srgb(0.24, 0.52, 0.32), 0.35);
        let cord = paint(Color::srgb(0.5, 0.42, 0.3), 0.0);
        commands.spawn((
            Mesh3d(frame),
            MeshMaterial3d(wood),
            Transform::default(),
            ChildOf(sign),
        ));
        // "Closed" faces the room at first, "open" is painted on the back.
        for (z, material) in [(BOARD.z * 0.5, closed), (-BOARD.z * 0.5, open)] {
            commands.spawn((
                Mesh3d(board.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.0, z).with_scale(Vec3::new(1.0, 1.0, 0.2)),
                ChildOf(sign),
            ));
        }
        let cord_mesh = meshes.add(Cylinder::new(0.008, 0.5));
        for x in [-0.25, 0.25] {
            commands.spawn((
                Mesh3d(cord_mesh.clone()),
                MeshMaterial3d(cord.clone()),
                Transform::from_xyz(x * 0.6, BOARD.y / 2.0 + 0.2, 0.0)
                    .with_rotation(Quat::from_rotation_z(x * 0.7)),
                NotShadowCaster,
                ChildOf(sign),
            ));
        }
    }
    commands
        .spawn((
            SignLabel,
            world_label(
                &font,
                lang.t("sign.closed"),
                13.0,
                sign,
                Vec3::new(0.0, 0.1, 0.05),
            ),
        ))
        .insert((
            text_font(&font.display, 13.0),
            TextShadow {
                offset: Vec2::splat(1.0),
                ..default()
            },
        ));
}

fn on_sign_click(
    click: On<PointerClick>,
    phase: Res<State<Phase>>,
    mut actions: MessageWriter<Action>,
) {
    if click.button == PointerButton::Primary && *phase.get() == Phase::Evening {
        actions.write(Action::StartNight);
    }
}

/// The sign or the button: back into the shop if needed, then open.
fn start_opening(
    mut actions: MessageReader<Action>,
    mut opening: ResMut<Opening>,
    mut location: ResMut<Location>,
    mut cues: MessageWriter<Cue>,
) {
    if actions.read().any(|a| *a == Action::StartNight) && opening.0.is_none() {
        opening.0 = Some(0.0);
        if *location != Location::Shop {
            *location = Location::Shop;
        }
        cues.write(Cue::Click);
    }
}

/// Runs the clock on to midnight, lets it strike, then opens the shop.
fn run_opening(
    time: Res<Time>,
    mut opening: ResMut<Opening>,
    mut clock_override: ResMut<ClockOverride>,
    mut cues: MessageWriter<Cue>,
    mut next: ResMut<NextState<Phase>>,
) {
    let Some(t) = opening.0 else { return };
    let before = t;
    let t = t + time.delta_secs();
    opening.0 = Some(t);
    let from = shop_minutes(Phase::Evening, &default());
    let k = ((t - CLOCK_FROM) / CLOCK_SECS).clamp(0.0, 1.0);
    // Slow start, quick middle, a little settle at the end.
    let eased = k * k * (3.0 - 2.0 * k);
    clock_override.0 = Some(from + (0.0 - from) * eased);
    if before < CLOCK_FROM + CLOCK_SECS && t >= CLOCK_FROM + CLOCK_SECS {
        cues.write(Cue::Midnight);
    }
    if t >= OPEN_AT {
        next.set(Phase::Night);
    }
}

/// The sign turns to "open" while the shop opens and stays so at night;
/// it swings a little when the mouse is over it.
fn turn_sign(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    opening: Res<Opening>,
    hovered: Res<SignHovered>,
    mut signs: Query<(&mut Sign, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let open = *phase.get() == Phase::Night || opening.0.is_some();
    for (mut sign, mut transform) in &mut signs {
        let target = if open { 1.0 } else { 0.0 };
        let step = dt / FLIP_SECS;
        sign.turn += (target - sign.turn).clamp(-step, step);
        // An overshoot makes the flip look like it swings round.
        let k = sign.turn;
        let angle = std::f32::consts::PI * (k + 0.12 * (k * std::f32::consts::PI).sin());
        let can_click = *phase.get() == Phase::Evening && !open;
        let swing = if hovered.0 && can_click {
            0.08 * (t * 6.0).sin()
        } else {
            0.015 * (t * 1.3).sin()
        };
        transform.rotation = Quat::from_rotation_y(angle) * Quat::from_rotation_z(swing);
    }
}

/// "Closed" / "open" on the sign while it faces the room; faded mid-flip.
fn update_sign_label(
    lang: Res<Lang>,
    font: Res<UiFont>,
    signs: Query<&Sign>,
    mut labels: Query<(&mut Text, &mut TextColor, &mut TextFont), With<SignLabel>>,
) {
    let Some(sign) = signs.iter().next() else {
        return;
    };
    for (mut text, mut color, mut face) in &mut labels {
        if font.is_changed() {
            face.font = font.display.clone();
        }
        let key = if sign.turn > 0.5 {
            "sign.open"
        } else {
            "sign.closed"
        };
        let wanted = lang.t(key);
        if text.0 != wanted {
            text.0 = wanted;
        }
        // (`follow_labels` owns the visibility, so fade with the colour.)
        let alpha = (((sign.turn - 0.5).abs() - 0.25) * 6.0).clamp(0.0, 1.0);
        if (color.0.alpha() - alpha).abs() > 0.01 {
            color.0 = Color::srgba(1.0, 0.95, 0.85, alpha);
        }
    }
}

// The clock strikes and the sign has turned before the night begins.
const _: () = assert!(CLOCK_FROM + CLOCK_SECS < OPEN_AT && FLIP_SECS < OPEN_AT);
