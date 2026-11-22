//! High contrast (Settings → *Seeing and moving*): the panels get a solid,
//! darker background, body and hint text turn brighter and buttons get light
//! edges (`ButtonTone::paint`). It applies to every panel as it is built, so
//! no panel needs to know about it, and switching it off restores the normal
//! look.

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

use crate::save::Settings;
use crate::ui::{BG_PANEL, ButtonTone, TEXT, TEXT_DIM};

/// Brighter text and the solid panel background in high contrast.
const HC_TEXT: Color = Color::srgb(1.0, 0.98, 0.94);
const HC_DIM: Color = Color::srgb(0.92, 0.88, 0.82);
const HC_PANEL: Color = Color::srgb(0.035, 0.025, 0.04);

static HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);

/// Is high contrast on? (Read while painting buttons.)
pub fn on() -> bool {
    HIGH_CONTRAST.load(Ordering::Relaxed)
}

pub struct ContrastPlugin;

impl Plugin for ContrastPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, (switch, recolor).chain());
    }
}

/// The painted frame of a panel (`UiTheme::panel_body`); darkened in high
/// contrast so text stands out against it.
#[derive(Component)]
pub struct PanelFrame;

/// How dark the painted frame becomes in high contrast.
const HC_FRAME_TINT: Color = Color::srgb(0.22, 0.2, 0.22);

/// A panel background made solid, with the transparency it had.
#[derive(Component)]
struct PanelAlpha(f32);

fn is_panel(color: Color) -> bool {
    let (a, b) = (color.to_srgba(), BG_PANEL.to_srgba());
    (a.red - b.red).abs() < 0.005
        && (a.green - b.green).abs() < 0.005
        && (a.blue - b.blue).abs() < 0.005
}

/// The text colour in the current mode.
fn text_color(color: Color, high: bool) -> Color {
    match (high, color) {
        (true, c) if c == TEXT => HC_TEXT,
        (true, c) if c == TEXT_DIM => HC_DIM,
        (false, c) if c == HC_TEXT => TEXT,
        (false, c) if c == HC_DIM => TEXT_DIM,
        (_, c) => c,
    }
}

/// The setting changed: repaint every button.
fn switch(
    settings: Res<Settings>,
    mut buttons: Query<(&ButtonTone, &mut BackgroundGradient, &mut BorderColor)>,
) {
    if !settings.is_changed() || settings.high_contrast == on() {
        return;
    }
    HIGH_CONTRAST.store(settings.high_contrast, Ordering::Relaxed);
    for (tone, mut bg, mut edge) in &mut buttons {
        (*bg, *edge) = tone.paint(false);
    }
}

/// New and changed text and panels follow the mode; all of them when it was
/// just switched.
fn recolor(
    mut commands: Commands,
    settings: Res<Settings>,
    mut texts: Query<&mut TextColor>,
    mut panels: Query<(Entity, &mut BackgroundColor, Option<&PanelAlpha>)>,
    mut frames: Query<Ref<PanelFrame>>,
    mut frame_images: Query<(&PanelFrame, &mut ImageNode)>,
) {
    let high = on();
    let all = settings.is_changed();
    let fresh = frames.iter_mut().any(|f| f.is_added());
    if all || fresh {
        let tint = if high { HC_FRAME_TINT } else { Color::WHITE };
        for (_, mut image) in &mut frame_images {
            if image.color != tint {
                image.color = tint;
            }
        }
    }
    for mut color in &mut texts {
        if all || color.is_changed() {
            let wanted = text_color(color.0, high);
            if wanted != color.0 {
                color.0 = wanted;
            }
        }
    }
    for (entity, mut bg, alpha) in &mut panels {
        if !(all || bg.is_changed()) {
            continue;
        }
        match (high, alpha) {
            (true, None) if is_panel(bg.0) && bg.0 != HC_PANEL => {
                commands.entity(entity).insert(PanelAlpha(bg.0.alpha()));
                bg.0 = HC_PANEL;
            }
            (false, Some(PanelAlpha(a))) => {
                bg.0 = BG_PANEL.with_alpha(*a);
                commands.entity(entity).remove::<PanelAlpha>();
            }
            _ => {}
        }
    }
}
