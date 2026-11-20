//! Helpers for trailers and store screenshots, only with the launch option
//! `--trailer` (players never see them):
//!
//! - **F9** hides or shows the whole interface (panels, log, top bar, names).
//! - **F10** starts or stops a slow camera drive around the current view and
//!   puts the camera back where it was when stopped.
//! - **F12** saves a screenshot of the window to `screenshots/` beside the
//!   working directory.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use bevy::camera::visibility::VisibilitySystems;

use crate::scene::MainCamera;

/// The launch option that turns the helpers on.
pub const FLAG: &str = "--trailer";

/// One full sway of the camera drive, in seconds.
const DRIVE_PERIOD: f32 = 36.0;
/// How far the camera drifts sideways (each way) and moves in.
const DRIVE_SIDEWAYS: f32 = 1.4;
const DRIVE_PUSH_IN: f32 = 1.6;
/// How far in front of the camera the point it keeps looking at lies.
const DRIVE_FOCUS: f32 = 14.0;
/// Seconds to ease into and out of the drive.
const DRIVE_EASE: f32 = 3.0;

const SCREENSHOT_DIR: &str = "screenshots";

pub struct TrailerPlugin;

impl Plugin for TrailerPlugin {
    fn build(&self, app: &mut App) {
        if !std::env::args().any(|a| a == FLAG) {
            return;
        }
        app.init_resource::<Trailer>()
            .add_systems(Update, (keys, drive_camera).chain())
            // After everything else, so a tooltip that shows itself stays hidden.
            .add_systems(
                PostUpdate,
                hide_interface.before(VisibilitySystems::VisibilityPropagate),
            );
    }
}

#[derive(Resource, Default)]
struct Trailer {
    hide_ui: bool,
    /// The camera's transform when the drive started, and the seconds since.
    drive: Option<(Transform, f32)>,
}

/// A UI root hidden by F9, with its own visibility to restore.
#[derive(Component)]
struct HiddenForTrailer(Visibility);

fn keys(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut trailer: ResMut<Trailer>,
    mut camera: Single<&mut Transform, With<MainCamera>>,
) {
    if keyboard.just_pressed(KeyCode::F9) {
        trailer.hide_ui = !trailer.hide_ui;
    }
    if keyboard.just_pressed(KeyCode::F10) {
        trailer.drive = match trailer.drive.take() {
            Some((start, _)) => {
                **camera = start;
                None
            }
            None => Some((**camera, 0.0)),
        };
    }
    if keyboard.just_pressed(KeyCode::F12) {
        let _ = std::fs::create_dir_all(SCREENSHOT_DIR);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path = format!("{SCREENSHOT_DIR}/shot-{stamp}.png");
        info!("Screenshot: {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

/// Panels are rebuilt all the time, so every UI root is checked each frame.
fn hide_interface(
    mut commands: Commands,
    trailer: Res<Trailer>,
    mut visible: Query<
        (Entity, &mut Visibility),
        (With<Node>, Without<ChildOf>, Without<HiddenForTrailer>),
    >,
    mut hidden: Query<(Entity, &mut Visibility, &HiddenForTrailer), With<Node>>,
) {
    if trailer.hide_ui {
        for (entity, mut visibility) in &mut visible {
            commands
                .entity(entity)
                .insert(HiddenForTrailer(*visibility));
            *visibility = Visibility::Hidden;
        }
        for (_, mut visibility, _) in &mut hidden {
            *visibility = Visibility::Hidden;
        }
    } else {
        for (entity, mut visibility, before) in &mut hidden {
            *visibility = before.0;
            commands.entity(entity).remove::<HiddenForTrailer>();
        }
    }
}

/// A slow sideways sway with a gentle push in, always looking at the same
/// point; it eases in so the first frames do not jump.
fn drive_camera(
    time: Res<Time>,
    mut trailer: ResMut<Trailer>,
    mut camera: Single<&mut Transform, With<MainCamera>>,
) {
    let Some((start, t)) = &mut trailer.drive else {
        return;
    };
    *t += time.delta_secs();
    let ease = (*t / DRIVE_EASE).min(1.0);
    let ease = ease * ease * (3.0 - 2.0 * ease);
    let phase = *t / DRIVE_PERIOD * std::f32::consts::TAU;
    let forward = start.forward().as_vec3();
    let right = start.right().as_vec3();
    let focus = start.translation + forward * DRIVE_FOCUS;
    let offset =
        right * phase.sin() * DRIVE_SIDEWAYS + forward * (0.5 - 0.5 * phase.cos()) * DRIVE_PUSH_IN;
    let pos = start.translation + offset * ease;
    **camera = Transform::from_translation(pos).looking_at(focus, Vec3::Y);
}
