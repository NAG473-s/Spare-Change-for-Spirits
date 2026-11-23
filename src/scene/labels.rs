//! Screen-space nameplates and stock tickets attached to the diorama.

use bevy::prelude::*;
use bevy::text::{ComputedTextBlock, LineBreak};

use super::MainCamera;
use crate::day_cycle::Phase;
use crate::save::Settings;
use crate::ui::{SidePanel, TopBar, UiFont, UiTheme, text_font};

/// Place signs first, then names, then stock tickets when space is tight.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LabelKind {
    Sign,
    Name,
    Stock,
}

impl LabelKind {
    pub(crate) fn font(self, font: &UiFont) -> TextFont {
        match self {
            Self::Sign => text_font(&font.display, 14.0),
            Self::Name => text_font(&font.display, 13.0),
            Self::Stock => text_font(&font.bold, 12.0),
        }
    }

    /// Ink, backing and edge. The light paper tickets read differently from the enamel names.
    fn colors(self, high_contrast: bool) -> (Color, Color, Color) {
        if high_contrast {
            return (
                Color::srgb_u8(255, 247, 221),
                Color::srgb_u8(16, 15, 20),
                Color::srgb_u8(240, 214, 157),
            );
        }
        match self {
            Self::Sign => (
                Color::srgb_u8(255, 222, 155),
                Color::srgb_u8(49, 31, 31),
                Color::srgb_u8(157, 115, 68),
            ),
            Self::Name => (
                Color::srgb_u8(205, 237, 225),
                Color::srgb_u8(26, 47, 48),
                Color::srgb_u8(125, 161, 143),
            ),
            Self::Stock => (
                Color::srgb_u8(55, 37, 28),
                Color::srgb_u8(232, 205, 155),
                Color::srgb_u8(132, 94, 58),
            ),
        }
    }
}

#[derive(Component)]
pub(super) struct WorldLabel {
    target: Entity,
    offset: Vec3,
    kind: LabelKind,
}

/// Optional visibility control for a label whose subject can lose focus.
#[derive(Component, PartialEq)]
pub(crate) struct LabelEnabled(pub bool);

/// The caption may also carry a marker when its wording changes during play.
pub(crate) fn world_label(
    font: &UiFont,
    caption: impl Bundle,
    kind: LabelKind,
    target: Entity,
    offset: Vec3,
) -> impl Bundle {
    let (width, padding_x, padding_y, radius) = match kind {
        LabelKind::Sign => (150.0, 9.0, 3.0, 3.0),
        LabelKind::Name => (150.0, 7.0, 2.0, 9.0),
        LabelKind::Stock => (132.0, 4.0, 1.0, 3.0),
    };
    (
        WorldLabel {
            target,
            offset,
            kind,
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: px(width),
            max_width: px(width),
            padding: UiRect::axes(px(padding_x), px(padding_y)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(radius)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BoxShadow::new(
            Color::srgba(0.05, 0.03, 0.06, 0.40),
            px(0),
            px(1),
            px(0),
            px(3),
        ),
        GlobalZIndex(-1),
        Visibility::Hidden,
        Pickable::IGNORE,
        children![(
            caption,
            kind.font(font),
            TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
            Node {
                width: percent(100),
                min_width: px(0),
                ..default()
            },
            Pickable::IGNORE,
        )],
    )
}

/// Fit each plate to the shaped caption, including its actual localized font and UI scale.
/// Starting at the maximum width also gives wrapped text a usable width on its first frame.
pub(super) fn size_labels(
    captions: Query<(&ComputedTextBlock, &ChildOf), Changed<ComputedTextBlock>>,
    mut plates: Query<(&ComputedNode, &mut Node), With<WorldLabel>>,
) {
    for (block, parent) in &captions {
        let Ok((computed, mut plate)) = plates.get_mut(parent.parent()) else {
            continue;
        };
        let width = block.buffer().calculate_content_widths().max;
        if width > 0.0 {
            let inset = computed.content_inset();
            let outer = width.ceil() + inset.min_inset.x + inset.max_inset.x;
            let desired = px((outer * computed.inverse_scale_factor()).ceil());
            if plate.width != desired {
                plate.width = desired;
            }
        }
    }
}

pub(super) fn paint_labels(
    mut commands: Commands,
    settings: Res<Settings>,
    theme: Res<UiTheme>,
    labels: Query<(Entity, Ref<WorldLabel>, &Children)>,
) {
    for (entity, label, children) in &labels {
        if !(label.is_added() || settings.is_changed()) {
            continue;
        }
        let (ink, backing, edge) = label.kind.colors(settings.high_contrast);
        commands.entity(children[0]).insert(TextColor(ink));
        let mut plate = commands.entity(entity);
        plate.insert((BackgroundColor(backing), BorderColor::all(edge)));
        if label.kind == LabelKind::Sign {
            if settings.high_contrast {
                plate.remove::<ImageNode>();
            } else if let Some(frame) = theme.frame(0.06) {
                plate.insert(frame.with_visual_box(VisualBox::PaddingBox));
            }
        }
    }
}

const GAP: f32 = 2.0;

fn at_anchor(point: Vec2, size: Vec2, kind: LabelKind, bounds: Rect) -> Option<Rect> {
    if size.x > bounds.width() || size.y > bounds.height() {
        return None;
    }
    let y = match kind {
        LabelKind::Sign => point.y - size.y * 0.5,
        _ => point.y - size.y - GAP,
    };
    let at = Vec2::new(point.x - size.x * 0.5, y).clamp(bounds.min, bounds.max - size);
    Some(Rect::from_corners(at, at + size))
}

/// Keep each plate as close to its anchor as possible, preferring above on a tie.
fn clear_neighbours(preferred: Rect, occupied: &[Rect], bounds: Rect) -> Option<Rect> {
    let fits = |r: Rect| {
        occupied
            .iter()
            .all(|other| r.intersect(other.inflate(GAP)).is_empty())
    };
    if fits(preferred) {
        return Some(preferred);
    }
    let height = preferred.height();
    occupied
        .iter()
        .flat_map(|other| {
            let above = other.min.y - GAP;
            let below = other.max.y + GAP;
            [
                Rect::new(preferred.min.x, above - height, preferred.max.x, above),
                Rect::new(preferred.min.x, below, preferred.max.x, below + height),
            ]
        })
        .filter(|&r| r.min.y >= bounds.min.y && r.max.y <= bounds.max.y && fits(r))
        .min_by(|a, b| {
            let delta = |r: &Rect| r.min.y - preferred.min.y;
            delta(a)
                .abs()
                .total_cmp(&delta(b).abs())
                .then_with(|| (delta(a) > 0.).cmp(&(delta(b) > 0.)))
        })
}

pub(super) fn follow_labels(
    mut commands: Commands,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    targets: Query<(&GlobalTransform, Option<&InheritedVisibility>)>,
    captions: Query<&Text>,
    phase: Res<State<Phase>>,
    scale: Res<UiScale>,
    hud: Query<
        (&ComputedNode, &InheritedVisibility, Has<TopBar>),
        Or<(With<TopBar>, With<SidePanel>)>,
    >,
    mut labels: Query<(
        Entity,
        &WorldLabel,
        &Children,
        &ComputedNode,
        &mut UiTransform,
        &mut Visibility,
        Option<&LabelEnabled>,
    )>,
) {
    let (camera, camera_at) = *camera;
    let viewport = camera.logical_viewport_rect().unwrap_or_default();
    let mut bounds = Rect::from_corners(viewport.min / scale.0, viewport.max / scale.0);
    for (node, visibility, top_bar) in &hud {
        if visibility.get() {
            let size = node.size() * node.inverse_scale_factor();
            if top_bar {
                bounds.min.y += size.y + GAP;
            } else {
                bounds.max.x -= size.x + GAP;
            }
        }
    }
    let mut visible = Vec::new();
    for (entity, label, children, computed, _, mut visibility, enabled) in &mut labels {
        let Ok((target, visible_in_world)) = targets.get(label.target) else {
            commands.entity(entity).try_despawn();
            continue;
        };
        let size = computed.size() * computed.inverse_scale_factor();
        if *phase.get() == Phase::Menu
            || enabled.is_some_and(|flag| !flag.0)
            || size.min_element() <= 0.0
            || visible_in_world.is_some_and(|v| !v.get())
            || !captions.get(children[0]).is_ok_and(|t| !t.is_empty())
        {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        let point = camera.world_to_viewport(camera_at, target.translation() + label.offset);
        let Some(point) = point.ok().filter(|p| viewport.contains(*p)) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if let Some(at) = at_anchor(point / scale.0, size, label.kind, bounds) {
            visible.push((entity, label.kind, at));
        } else {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }

    // Front-to-back within each group keeps a queue of long names in reading order.
    visible.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| b.2.max.y.total_cmp(&a.2.max.y))
            .then_with(|| a.2.min.x.total_cmp(&b.2.min.x))
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut occupied = Vec::with_capacity(visible.len());
    for (entity, _, preferred) in visible {
        let (_, _, _, _, mut transform, mut visibility, _) = labels.get_mut(entity).unwrap();
        let Some(at) = clear_neighbours(preferred, &occupied, bounds) else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        occupied.push(at);
        transform.set_if_neq(UiTransform::from_xy(px(at.min.x), px(at.min.y)));
        visibility.set_if_neq(Visibility::Inherited);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_names_stack_at_their_target_and_fit_below_at_the_top_edge() {
        let screen = Rect::new(0., 0., 360., 200.);
        let first = Rect::new(50., 100., 190., 124.);
        let second = Rect::new(100., 100., 240., 124.);
        let raised = clear_neighbours(second, &[first], screen).unwrap();
        assert_eq!(raised.max.y, first.min.y - GAP);
        assert_eq!(raised.min.x, second.min.x);
        let third = clear_neighbours(second, &[first, raised], screen).unwrap();
        assert_eq!(third.min.y, first.max.y + GAP);

        let top = Rect::new(50., 1., 190., 25.);
        let below = clear_neighbours(top, &[top], screen).unwrap();
        assert_eq!(below.min.y, top.max.y + GAP);
        assert_eq!(clear_neighbours(first, &[top], screen), Some(first));
        assert_eq!(clear_neighbours(screen, &[first], screen), None);
    }

    #[test]
    fn enlarged_labels_stay_in_front_of_the_scene_beside_the_hud() {
        let area = Rect::new(0., 42., 472., 480.);
        let size = Vec2::new(112., 22.);
        let name = at_anchor(Vec2::new(521., 224.), size, LabelKind::Name, area).unwrap();
        assert_eq!(name.max, Vec2::new(area.max.x, 222.));
        let stock = at_anchor(Vec2::new(0., 38.), size, LabelKind::Stock, area).unwrap();
        assert_eq!(stock.min, area.min);
        assert!(at_anchor(Vec2::ZERO, Vec2::splat(500.), LabelKind::Sign, area).is_none());
    }

    #[test]
    fn small_scene_labels_have_high_text_contrast_in_both_modes() {
        let luminance = |c: Color| {
            let c = c.to_linear();
            0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue
        };
        for kind in [LabelKind::Name, LabelKind::Stock, LabelKind::Sign] {
            for high in [false, true] {
                let (ink, backing, _) = kind.colors(high);
                let (a, b) = (luminance(ink), luminance(backing));
                assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 7.0);
            }
        }
    }
}
