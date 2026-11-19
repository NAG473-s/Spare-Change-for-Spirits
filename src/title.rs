//! Title screen: a painted background of its own behind the main menu, with
//! flickering lamp light, drifting dust and now and then a ghost floating past.
//! It covers the shop completely while the game is in `Phase::Menu`.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::data::{GameData, asset_exists};
use crate::day_cycle::Phase;
use crate::rng::Rng;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Phase::Menu), spawn_title)
            .add_systems(OnExit(Phase::Menu), |mut commands: Commands| {
                spawn_fade(&mut commands, 0.8);
            })
            .add_systems(Update, fade_in)
            .add_systems(
                Update,
                (
                    cover_window,
                    breathe_background,
                    flicker_glows,
                    drift_motes,
                    float_ghost,
                    bob_logo,
                )
                    .run_if(in_state(Phase::Menu)),
            );
    }
}

const BACKGROUND: &str = "sprites/title/background.png";
/// The game logo shown above the main menu (text heading if it is missing).
pub const LOGO: &str = "sprites/title/logo.png";
/// Width / height of the background until its image has loaded.
const DEFAULT_ASPECT: f32 = 16.0 / 9.0;
/// Anchors in the painted image, including when the window crops the artwork.
const GLOWS: [(Vec2, f32, Glow); 3] = [
    (
        Vec2::new(0.528, 0.661),
        0.075,
        Glow {
            tint: Color::srgb(1.0, 0.66, 0.31),
            strength: 0.41,
            flutter: 0.19,
            speed: 7.3,
        },
    ),
    (
        Vec2::new(0.648, 0.204),
        0.125,
        Glow {
            tint: Color::srgb(1.0, 0.79, 0.50),
            strength: 0.25,
            flutter: 0.06,
            speed: 3.1,
        },
    ),
    (
        Vec2::new(0.832, 0.205),
        0.132,
        Glow {
            tint: Color::srgb(0.57, 0.73, 1.0),
            strength: 0.16,
            flutter: 0.025,
            speed: 0.42,
        },
    ),
];
const MOTES: usize = 28;
const GHOST_SIZE: Vec2 = Vec2::new(84.0, 126.0);
const FADE_SECS: f32 = 1.2;

/// Root of the title screen, under the menu.
#[derive(Component)]
struct TitleScreen;

/// The painted background, sized to cover the window.
#[derive(Component)]
struct TitleBackground(Handle<Image>);

#[derive(Component)]
struct Glow {
    tint: Color,
    strength: f32,
    flutter: f32,
    speed: f32,
}

#[derive(Component)]
struct TitleMote {
    pos: Vec2,
    speed: f32,
    phase: f32,
}

/// A ghost floating across the screen; waits between passes.
#[derive(Component)]
struct TitleGhost {
    /// Fraction of the way across, < 0 while waiting.
    progress: f32,
    speed: f32,
    height: f32,
    to_right: bool,
}

/// Bobs gently (the logo above the menu).
#[derive(Component)]
pub struct TitleLogo;

/// Black layer over everything that fades out: elapsed and total seconds.
#[derive(Component)]
struct FadeIn(f32, f32);

fn spawn_title(mut commands: Commands, asset_server: Res<AssetServer>, mut rng: ResMut<Rng>) {
    let image = asset_server.load(BACKGROUND);
    let root = commands
        .spawn((
            TitleScreen,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.04, 0.08)),
            GlobalZIndex(90),
            Pickable::IGNORE,
            DespawnOnExit(Phase::Menu),
        ))
        .id();

    let background = commands
        .spawn((
            TitleBackground(image.clone()),
            ImageNode {
                image,
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTransform::IDENTITY,
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    for (centre, radius, glow) in GLOWS {
        // Percentages of the background node: left/width relative to its
        // width, top/height relative to its height.
        let w = radius * 2.0;
        let h = w * DEFAULT_ASPECT;
        commands.spawn((
            BackgroundGradient(vec![
                RadialGradient {
                    shape: RadialGradientShape::ClosestSide,
                    stops: [0.0, 23.0, 57.0, 100.0]
                        .map(|p| ColorStop::new(glow.tint, percent(p)))
                        .to_vec(),
                    ..default()
                }
                .into(),
            ]),
            glow,
            Node {
                position_type: PositionType::Absolute,
                left: percent((centre.x - w / 2.0) * 100.0),
                top: percent((centre.y - h / 2.0) * 100.0),
                width: percent(w * 100.0),
                height: percent(h * 100.0),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(background),
        ));
    }

    // Darken the left side, where the logo and the menu sit, and the edges.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        BackgroundGradient(vec![
            LinearGradient::to_right(vec![
                ColorStop::new(Color::srgba(0.02, 0.02, 0.05, 0.78), percent(0)),
                ColorStop::new(Color::srgba(0.02, 0.02, 0.05, 0.45), percent(32)),
                ColorStop::new(Color::srgba(0.02, 0.02, 0.05, 0.0), percent(58)),
            ])
            .into(),
            LinearGradient::to_top(vec![
                ColorStop::new(Color::srgba(0.02, 0.02, 0.05, 0.55), percent(0)),
                ColorStop::new(Color::srgba(0.02, 0.02, 0.05, 0.0), percent(25)),
            ])
            .into(),
        ]),
        Pickable::IGNORE,
        ChildOf(root),
    ));

    for _ in 0..MOTES {
        let size = rng.range(2.0, 5.0);
        commands.spawn((
            TitleMote {
                pos: Vec2::new(rng.f32(), rng.f32()),
                speed: rng.range(0.008, 0.03),
                phase: rng.range(0.0, 6.3),
            },
            Node {
                position_type: PositionType::Absolute,
                width: px(size),
                height: px(size),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }

    commands.spawn((
        TitleGhost {
            progress: -0.3,
            speed: 0.0,
            height: 0.0,
            to_right: true,
        },
        ImageNode::default(),
        Node {
            position_type: PositionType::Absolute,
            width: px(GHOST_SIZE.x),
            height: px(GHOST_SIZE.y),
            ..default()
        },
        Visibility::Hidden,
        UiTransform::IDENTITY,
        Pickable::IGNORE,
        ChildOf(root),
    ));

    spawn_fade(&mut commands, FADE_SECS);
}

/// Fades in from black, over everything including the menu.
pub fn spawn_fade(commands: &mut Commands, secs: f32) {
    commands.spawn((
        FadeIn(0.0, secs),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        GlobalZIndex(200),
        Pickable::IGNORE,
    ));
}

/// Scales the background so it covers the whole window (cropping the
/// overhang), like CSS `background-size: cover`.
fn cover_window(
    window: Single<&Window, With<PrimaryWindow>>,
    images: Res<Assets<Image>>,
    mut backgrounds: Query<(&TitleBackground, &mut Node)>,
) {
    let size = Vec2::new(window.width(), window.height());
    for (background, mut node) in &mut backgrounds {
        let aspect = images
            .get(&background.0)
            .map(|image| image.aspect_ratio().ratio())
            .unwrap_or(DEFAULT_ASPECT);
        let width = size.x.max(size.y * aspect);
        let height = width / aspect;
        let (left, top) = ((size.x - width) / 2.0, (size.y - height) / 2.0);
        if node.width != px(width) || node.height != px(height) {
            node.width = px(width);
            node.height = px(height);
            node.left = px(left);
            node.top = px(top);
        }
    }
}

/// A very slow zoom in and out, so the picture feels alive.
fn breathe_background(
    time: Res<Time>,
    mut backgrounds: Query<&mut UiTransform, With<TitleBackground>>,
) {
    let t = time.elapsed_secs();
    for mut transform in &mut backgrounds {
        transform.scale = Vec2::splat(1.025 + 0.02 * (t * 0.12).sin());
    }
}

fn flicker_glows(time: Res<Time>, mut glows: Query<(&Glow, &mut BackgroundGradient)>) {
    let t = time.elapsed_secs();
    for (glow, mut gradient) in &mut glows {
        let phase = t * glow.speed;
        let pulse = 0.64 * phase.sin() + 0.36 * (phase * 1.79 + 0.7).sin();
        let alpha = glow.strength * (1.0 + glow.flutter * pulse);
        if let [Gradient::Radial(radial)] = gradient.0.as_mut_slice() {
            for (stop, weight) in radial.stops.iter_mut().zip([1.0, 0.60, 0.12, 0.0]) {
                stop.color.set_alpha(alpha * weight);
            }
        }
    }
}

/// Dust rises slowly through the lamp light and twinkles.
fn drift_motes(
    time: Res<Time>,
    mut motes: Query<(&mut TitleMote, &mut Node, &mut BackgroundColor)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut mote, mut node, mut color) in &mut motes {
        mote.pos.y -= mote.speed * dt;
        if mote.pos.y < -0.02 {
            mote.pos.y = 1.02;
        }
        let x = mote.pos.x + 0.012 * (t * 0.4 + mote.phase).sin();
        node.left = percent(x * 100.0);
        node.top = percent(mote.pos.y * 100.0);
        let twinkle = 0.5 + 0.5 * (t * 1.3 + mote.phase * 3.0).sin();
        color.0 = Color::srgba(1.0, 0.86, 0.6, 0.12 + 0.4 * twinkle);
    }
}

/// Now and then one of the shop's ghosts floats across the top of the screen.
fn float_ghost(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    data: Res<GameData>,
    asset_server: Res<AssetServer>,
    mut rng: ResMut<Rng>,
    mut ghosts: Query<(
        &mut TitleGhost,
        &mut ImageNode,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
    )>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let width = window.width();
    for (mut ghost, mut image, mut node, mut transform, mut visibility) in &mut ghosts {
        if ghost.progress < 0.0 {
            // Waiting: a negative progress counts up to the next pass.
            ghost.progress += dt * 0.1;
            *visibility = Visibility::Hidden;
            if ghost.progress < 0.0 {
                continue;
            }
            let ids: Vec<&str> = data
                .ghosts
                .iter()
                .map(|g| g.id.as_str())
                .filter(|id| asset_exists(&format!("sprites/ghosts/{id}.png")))
                .collect();
            if ids.is_empty() {
                ghost.progress = -1.0;
                continue;
            }
            let id = ids[rng.index(ids.len())];
            image.image = asset_server.load(format!("sprites/ghosts/{id}.png"));
            ghost.progress = 0.0;
            ghost.speed = rng.range(0.035, 0.05);
            ghost.height = rng.range(0.05, 0.3);
            ghost.to_right = rng.chance(0.5);
        }
        ghost.progress += ghost.speed * dt;
        if ghost.progress > 1.0 {
            ghost.progress = -rng.range(0.4, 1.0);
            continue;
        }
        *visibility = Visibility::Inherited;
        let p = if ghost.to_right {
            ghost.progress
        } else {
            1.0 - ghost.progress
        };
        let x = -GHOST_SIZE.x + p * (width + GHOST_SIZE.x);
        let bob = 10.0 * (t * 1.7).sin();
        node.left = px(x);
        node.top = percent(ghost.height * 100.0);
        transform.translation = Val2::px(0.0, bob);
        // Sprites are drawn looking right (as in the shop); mirror them to float left.
        let flip = if ghost.to_right { 1.0 } else { -1.0 };
        transform.scale = Vec2::new(flip, 1.0);
        transform.rotation = Rot2::radians(0.06 * (t * 1.1).sin());
        // Fade in and out at the edges, and stay half transparent.
        let edge = (ghost.progress * 6.0)
            .min((1.0 - ghost.progress) * 6.0)
            .min(1.0);
        image.color = Color::srgba(1.0, 1.0, 1.0, 0.7 * edge);
    }
}

fn bob_logo(time: Res<Time>, mut logos: Query<&mut UiTransform, With<TitleLogo>>) {
    let t = time.elapsed_secs();
    for mut transform in &mut logos {
        transform.translation = Val2::px(0.0, 4.0 * (t * 1.2).sin());
    }
}

fn fade_in(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut fades: Query<(Entity, &mut FadeIn, &mut BackgroundColor)>,
) {
    for (entity, mut fade, mut color) in &mut fades {
        fade.0 += time.delta_secs();
        let a = 1.0 - (fade.0 / fade.1).min(1.0);
        color.0 = Color::srgba(0.0, 0.0, 0.0, a * a);
        if a <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
