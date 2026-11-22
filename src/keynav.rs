//! Playing with the keyboard alone: Tab and the arrow keys move a focus
//! frame over the buttons of the topmost layer (the side panel, or the book,
//! the pause menu or a caption above it), Enter or Space presses the framed
//! button, Escape opens the pause menu (`menu.rs`).
//!
//! Every action of the shop has a button: the crates on the ferry and the
//! mishaps of the night are in the panels as well as in the scene, and the
//! selected ware or piece of furniture can be put on the first shelf or square
//! with room (`shop::use_tool_anywhere`). The focus is Bevy's `InputFocus`, so
//! the standard button widget turns Enter and Space into a click. Moving the
//! mouse hides the frame until the next key.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;

use crate::ui::ActionButton;

pub struct KeyNavPlugin;

impl Plugin for KeyNavPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KeyNav>()
            .add_systems(Update, (navigate, hide_on_mouse, show_frame).chain());
    }
}

/// Width of the focus frame (logical pixels), drawn just inside the button
/// so scrolling panels do not clip it, and its colour: a bright frame that
/// reads on dark wood and on brass buttons alike.
const FRAME_WIDTH: f32 = 3.0;
const FRAME_OFFSET: f32 = -1.0;
const FRAME_COLOR: Color = Color::srgb(1.0, 0.93, 0.55);
/// Buttons whose tops lie within this many pixels count as one row.
const ROW_TOLERANCE: f32 = 6.0;

/// Where the keyboard focus is, and what it was, so it can find its way
/// back after a panel is rebuilt (the buttons are new entities then).
#[derive(Resource, Default)]
struct KeyNav {
    /// The frame is shown (a key was pressed since the mouse last moved).
    visible: bool,
    last_action: Option<crate::ui::Action>,
    last_pos: Vec2,
}

/// A button that can take the focus now: its entity, action, top-left
/// corner on screen and layer.
struct Candidate {
    entity: Entity,
    action: crate::ui::Action,
    pos: Vec2,
    layer: i32,
}

/// The layer of a node: the highest `GlobalZIndex` among it and its parents.
fn layer_of(mut entity: Entity, parents: &Query<&ChildOf>, z: &Query<&GlobalZIndex>) -> i32 {
    let mut layer = 0;
    loop {
        if let Ok(GlobalZIndex(i)) = z.get(entity) {
            layer = layer.max(*i);
        }
        match parents.get(entity) {
            Ok(parent) => entity = parent.parent(),
            Err(_) => return layer,
        }
    }
}

/// The enabled, visible buttons of the topmost layer, in reading order.
fn candidates(
    buttons: &Query<(
        Entity,
        &ActionButton,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Has<InteractionDisabled>,
    )>,
    parents: &Query<&ChildOf>,
    z: &Query<&GlobalZIndex>,
) -> Vec<Candidate> {
    let mut all: Vec<Candidate> = buttons
        .iter()
        .filter(|(_, _, node, _, visible, disabled)| {
            !disabled && visible.get() && node.size().x > 0.0 && node.size().y > 0.0
        })
        .map(|(entity, action, node, transform, _, _)| {
            let scale = node.inverse_scale_factor();
            let top_left = (transform.translation - node.size() / 2.0) * scale;
            Candidate {
                entity,
                action: action.0.clone(),
                pos: top_left,
                layer: layer_of(entity, parents, z),
            }
        })
        .collect();
    let top = all.iter().map(|c| c.layer).max().unwrap_or(0);
    all.retain(|c| c.layer == top);
    all.sort_by(|a, b| {
        if (a.pos.y - b.pos.y).abs() <= ROW_TOLERANCE {
            a.pos.x.total_cmp(&b.pos.x)
        } else {
            a.pos.y.total_cmp(&b.pos.y)
        }
    });
    all
}

#[allow(clippy::type_complexity)]
fn navigate(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(
        Entity,
        &ActionButton,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Has<InteractionDisabled>,
    )>,
    parents: Query<&ChildOf>,
    z: Query<&GlobalZIndex>,
    mut focus: ResMut<InputFocus>,
    mut nav: ResMut<KeyNav>,
) {
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let step: i32 = if keys.just_pressed(KeyCode::Tab) {
        if shift { -1 } else { 1 }
    } else if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::ArrowRight]) {
        1
    } else if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::ArrowLeft]) {
        -1
    } else {
        0
    };
    let list = candidates(&buttons, &parents, &z);
    // The focused button is gone or hidden (a panel was rebuilt, an overlay
    // opened): find the same action again, or the nearest button.
    let current = focus
        .get()
        .and_then(|e| list.iter().position(|c| c.entity == e));
    let current = current.or_else(|| {
        if focus.get().is_none() && nav.last_action.is_none() {
            return None;
        }
        let found = nav
            .last_action
            .as_ref()
            .and_then(|a| list.iter().position(|c| c.action == *a))
            .or_else(|| {
                (0..list.len()).min_by(|&a, &b| {
                    let d = |i: usize| list[i].pos.distance_squared(nav.last_pos);
                    d(a).total_cmp(&d(b))
                })
            });
        if let Some(i) = found {
            focus.set(list[i].entity, FocusCause::Navigated);
        } else {
            focus.clear();
        }
        found
    });
    if step == 0 || list.is_empty() {
        if let Some(i) = current {
            nav.last_pos = list[i].pos;
        }
        return;
    }
    let was_visible = std::mem::replace(&mut nav.visible, true);
    let len = list.len() as i32;
    let next = match current {
        // The first key shows the frame where it is.
        Some(i) if !was_visible => i as i32,
        Some(i) => (i as i32 + step).rem_euclid(len),
        None if step > 0 => 0,
        None => len - 1,
    } as usize;
    let chosen = &list[next];
    focus.set(chosen.entity, FocusCause::Navigated);
    nav.last_action = Some(chosen.action.clone());
    nav.last_pos = chosen.pos;
}

/// The mouse is back: hide the frame (the focus stays for the next key).
fn hide_on_mouse(
    mut moved: MessageReader<bevy::window::CursorMoved>,
    mut clicked: MessageReader<bevy::input::mouse::MouseButtonInput>,
    mut nav: ResMut<KeyNav>,
) {
    let mouse = moved.read().count() + clicked.read().count() > 0;
    if mouse && nav.visible {
        nav.visible = false;
    }
}

#[derive(Component)]
struct Framed;

/// The frame around the focused button, and the button scrolled into view.
#[allow(clippy::type_complexity)]
fn show_frame(
    mut commands: Commands,
    nav: Res<KeyNav>,
    focus: Res<InputFocus>,
    framed: Query<Entity, With<Framed>>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    mut scrolls: Query<(&mut ScrollPosition, &Node)>,
) {
    let target = focus.get().filter(|_| nav.visible);
    for e in &framed {
        if Some(e) != target {
            commands.entity(e).try_remove::<(Framed, Outline)>();
        }
    }
    let Some(target) = target else {
        return;
    };
    if !framed.contains(target) {
        commands.entity(target).try_insert((
            Framed,
            Outline::new(px(FRAME_WIDTH), px(FRAME_OFFSET), FRAME_COLOR),
        ));
    }
    if !focus.is_changed() {
        return;
    }
    // Scroll the nearest scrolling parent so the button is fully visible.
    let Ok((node, transform)) = nodes.get(target) else {
        return;
    };
    let half = node.size().y / 2.0;
    let (top, bottom) = (
        transform.translation.y - half,
        transform.translation.y + half,
    );
    let mut entity = target;
    while let Ok(parent) = parents.get(entity) {
        entity = parent.parent();
        let Ok((mut scroll, style)) = scrolls.get_mut(entity) else {
            continue;
        };
        if style.overflow.y != OverflowAxis::Scroll {
            continue;
        }
        let Ok((view, at)) = nodes.get(entity) else {
            break;
        };
        let scale = view.inverse_scale_factor();
        let view_half = view.size().y / 2.0;
        let (view_top, view_bottom) = (at.translation.y - view_half, at.translation.y + view_half);
        if top < view_top {
            scroll.y -= (view_top - top) * scale;
        } else if bottom > view_bottom {
            scroll.y += (bottom - view_bottom) * scale;
        }
        scroll.y = scroll.y.max(0.0);
        break;
    }
}
