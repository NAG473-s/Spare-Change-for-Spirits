//! A tooltip that follows the mouse pointer, for things in the 3D world that
//! say more about themselves when hovered (wares at the market, products on
//! the shelves). Whoever owns the hovered thing fills `HoverTip`; empty means
//! hidden.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::ui::{BG_PANEL, PanelBuilder, TEXT, TEXT_DIM, UiFont, UiTheme};

pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoverTip>()
            .add_systems(Startup, spawn_tooltip)
            .add_systems(PostUpdate, show_tooltip);
    }
}

/// What the tooltip shows. Set it while something is hovered and clear it
/// when the pointer leaves.
#[derive(Resource, Default, Clone, PartialEq)]
pub struct HoverTip {
    pub icon: Option<Handle<Image>>,
    pub title: String,
    pub lines: Vec<TipLine>,
}

#[derive(Clone, PartialEq)]
pub struct TipLine {
    pub text: String,
    pub kind: TipKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TipKind {
    Text,
    Good,
    Bad,
    Hint,
}

impl HoverTip {
    pub fn is_empty(&self) -> bool {
        self.title.is_empty()
    }

    pub fn line(&mut self, kind: TipKind, text: String) -> &mut Self {
        self.lines.push(TipLine { text, kind });
        self
    }

    /// Replaces the shown content only if it differs (so the tooltip is
    /// not rebuilt every frame).
    pub fn show(target: &mut ResMut<HoverTip>, tip: HoverTip) {
        if **target != tip {
            **target = tip;
        }
    }

    pub fn hide(target: &mut ResMut<HoverTip>) {
        if !target.is_empty() {
            **target = HoverTip::default();
        }
    }
}

const WIDTH: f32 = 290.0;
const GOOD: Color = Color::srgb(0.55, 0.85, 0.6);
const BAD: Color = Color::srgb(0.95, 0.5, 0.45);
const TITLE: Color = Color::srgb(0.98, 0.78, 0.52);

/// The tooltip frame; its body holds the text.
#[derive(Component)]
struct TooltipFrame(Entity);

fn spawn_tooltip(mut commands: Commands, theme: Res<UiTheme>) {
    let frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(WIDTH),
                max_width: percent(94),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            GlobalZIndex(150),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .id();
    let body = theme.panel_body(&mut commands, frame, 0.23, 13.0, 4.0);
    commands.entity(body).insert(Pickable::IGNORE);
    commands.entity(frame).insert(TooltipFrame(body));
}

fn show_tooltip(
    mut commands: Commands,
    tip: Res<HoverTip>,
    font: Res<UiFont>,
    window: Single<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut frame: Single<(&TooltipFrame, &ComputedNode, &mut Node, &mut Visibility)>,
) {
    let (frame, computed, node, visibility) = &mut *frame;
    let Some(cursor) = window.cursor_position().filter(|_| !tip.is_empty()) else {
        **visibility = Visibility::Hidden;
        return;
    };
    **visibility = Visibility::Inherited;
    // In interface units: window pixels divided by the interface scale.
    let s = ui_scale.0;
    let (cursor, win) = (cursor / s, Vec2::new(window.width(), window.height()) / s);
    let width = WIDTH.min(win.x * 0.94);
    let height = computed.size().y * computed.inverse_scale_factor();
    // Beside the pointer, and always on screen.
    let x = if cursor.x + width + 26.0 <= win.x {
        cursor.x + 18.0
    } else {
        cursor.x - width - 18.0
    };
    node.left = px(x.clamp(8.0, (win.x - width - 8.0).max(8.0)));
    node.top = px((cursor.y - height * 0.65).clamp(8.0, (win.y - height - 8.0).max(8.0)));

    if !tip.is_changed() {
        return;
    }
    let body = frame.0;
    let mut ui = PanelBuilder::new(&mut commands, body, &font);
    let heading = ui.node(
        body,
        (
            Node {
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
        ),
    );
    if let Some(icon) = &tip.icon {
        ui.image_sized(heading, icon.clone(), Vec2::splat(32.0));
    }
    ui.display_text(heading, &tip.title, 17.0, TITLE);
    if !tip.lines.is_empty() {
        ui.rule();
    }
    for line in &tip.lines {
        let (size, color) = match line.kind {
            TipKind::Text => (14.0, TEXT),
            TipKind::Good => (14.0, GOOD),
            TipKind::Bad => (13.0, BAD),
            TipKind::Hint => (12.0, TEXT_DIM),
        };
        ui.add_text(body, &line.text, false, size, color);
    }
}
