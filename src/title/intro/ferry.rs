//! Layer the painted cutouts on the hull and reflect the same pose in the harbour.
use super::{
    Crossing, rect,
    stroke::{CANVAS, COUNT, Part, Stroke, WATER},
};
use bevy::prelude::*;

pub(super) const HULL: &str = "sprites/title/intro_ferry.png";
pub(super) const CREW: &str = "sprites/title/intro_crew.png";
pub(super) const GLOW: &str = "sprites/fx/glow.png";

const BOAT_WIDTH: f32 = 44.;
const REFLECTION_DEPTH: f32 = 0.29;
const REFLECTION: Color = Color::srgba(0.52, 0.65, 0.76, 0.18);
const RINGS: [(f32, f32); 6] = [
    (0., 0.),
    (48., 0.),
    (56., 0.35),
    (67., 0.),
    (82., 1.),
    (99., 0.),
];

#[derive(Component)]
pub(super) struct Ferry;
#[derive(Component)]
pub(super) struct Impact(u8);
#[derive(Component)]
pub(super) struct Lamplight(f32);

impl Part {
    /// Pixels in the 384 × 288 crew sheet, including padding around each brushstroke.
    fn cutout(self) -> Option<Rect> {
        let (x, y, w, h) = match self {
            Self::Hull => return None,
            Self::Body => (2., 2., 160., 222.),
            Self::FarArm => (168., 78., 152., 69.),
            Self::NearArm => (168., 2., 152., 70.),
            Self::Staff => (2., 270., 376., 14.),
            Self::Lamp => (168., 153., 82., 110.),
        };
        Some(Rect::new(x, y, x + w, y + h))
    }
}

fn bounds(centre: Vec2, size: Vec2) -> Node {
    let corner = (centre - size * 0.5) / CANVAS * 100.;
    let extent = size / CANVAS * 100.;
    rect(corner.x, corner.y, extent.x, extent.y)
}

pub(super) fn spawn(
    commands: &mut Commands,
    root: Entity,
    hull: Option<Handle<Image>>,
    crew: Option<Handle<Image>>,
    glow: Option<Handle<Image>>,
) {
    let ferry = commands
        .spawn((
            Ferry,
            Node {
                position_type: PositionType::Absolute,
                left: percent(4.5),
                top: percent(80),
                width: percent(BOAT_WIDTH),
                max_width: vh(86),
                aspect_ratio: Some(CANVAS.x / CANVAS.y),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(0., -100. * WATER / CANVAS.y)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let start = Stroke::at(0.);
    for mirrored in [true, false] {
        // Mirror the whole assembled puppet around its waterline, including its pole.
        let transform = if mirrored {
            UiTransform {
                scale: Vec2::new(1., -REFLECTION_DEPTH),
                translation: Val2::percent(
                    0.,
                    (1. + REFLECTION_DEPTH) * (WATER / CANVAS.y - 0.5) * 100.,
                ),
                ..default()
            }
        } else {
            UiTransform::IDENTITY
        };
        let layer = commands
            .spawn((
                rect(0., 0., 100., 100.),
                transform,
                Pickable::IGNORE,
                ChildOf(ferry),
            ))
            .id();
        for part in [
            Part::Hull,
            Part::Body,
            Part::Staff,
            Part::FarArm,
            Part::NearArm,
            Part::Lamp,
        ] {
            let pose = start.pose(part);
            let id = commands
                .spawn((
                    part,
                    bounds(pose.centre, pose.size),
                    UiTransform::from_rotation(pose.angle),
                    Pickable::IGNORE,
                    ChildOf(layer),
                ))
                .id();
            let cutout = part.cutout();
            let texture = if cutout.is_some() { &crew } else { &hull };
            if let Some(image) = texture {
                commands.entity(id).insert(ImageNode {
                    image: image.clone(),
                    rect: cutout,
                    image_mode: NodeImageMode::Stretch,
                    color: if mirrored { REFLECTION } else { Color::WHITE },
                    ..default()
                });
            } else {
                placeholder(commands, id, part, mirrored);
            }
            if let (Part::Lamp, Some(image)) = (part, glow.as_ref()) {
                let intensity = if mirrored { 0.20 } else { 1. };
                commands.spawn((
                    Lamplight(intensity),
                    ImageNode {
                        image: image.clone(),
                        color: Color::srgba(1., 0.72, 0.32, 0.35 * intensity),
                        image_mode: NodeImageMode::Stretch,
                        ..default()
                    },
                    rect(-57., -12., 214., 158.),
                    Pickable::IGNORE,
                    ChildOf(id),
                ));
            }
        }
    }
    for i in 0..COUNT {
        let stops = RINGS
            .iter()
            .map(|&(offset, _)| ColorStop::new(Color::NONE, percent(offset)))
            .collect();
        commands.spawn((
            Impact(i),
            Node::default(),
            BackgroundGradient(vec![
                RadialGradient {
                    shape: RadialGradientShape::Ellipse(percent(50), percent(50)),
                    stops,
                    ..default()
                }
                .into(),
            ]),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(ferry),
        ));
    }
}

/// Keep the same joints, contact point and hook when a painting is unavailable.
fn placeholder(commands: &mut Commands, id: Entity, part: Part, mirrored: bool) {
    let wood = Color::srgb(0.36, 0.23, 0.19);
    let brass = Color::srgb(0.82, 0.61, 0.35);
    let ink = Color::srgb(0.20, 0.16, 0.25);
    let mut shape = |x, y, w, h, tint: Color| {
        commands.spawn((
            Node {
                border_radius: BorderRadius::all(percent(28)),
                ..rect(x, y, w, h)
            },
            BackgroundColor(if mirrored { REFLECTION } else { tint }),
            Pickable::IGNORE,
            ChildOf(id),
        ));
    };
    match part {
        Part::Hull => {
            shape(1., 68., 97., 24., wood);
            shape(85., 12., 1., 57., brass);
            shape(85., 12., 7., 1.8, brass);
        }
        Part::Body => {
            shape(16., 2., 63., 91., ink);
            shape(55., 17., 13., 3., brass);
        }
        Part::FarArm | Part::NearArm => shape(4., 18., 92., 65., ink),
        Part::Staff => shape(0., 16., 100., 68., wood),
        Part::Lamp => shape(22., 14., 58., 77., Color::srgb(1., 0.76, 0.33)),
    }
}

pub(super) fn animate(
    crossing: Query<&Crossing>,
    mut track: Query<&mut UiTransform, (With<Ferry>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Node, &mut UiTransform)>,
    mut rings: Query<(&Impact, &mut Node, &mut Visibility, &mut BackgroundGradient), Without<Part>>,
    mut lamps: Query<(&Lamplight, &mut ImageNode)>,
) {
    let Ok(clock) = crossing.single() else { return };
    let stroke = Stroke::at(clock.elapsed);
    for mut transform in &mut track {
        transform.translation.x = percent(100. * stroke.travel / CANVAS.x);
    }
    for (part, mut node, mut transform) in &mut parts {
        let pose = stroke.pose(*part);
        *node = bounds(pose.centre, pose.size);
        transform.rotation = pose.angle;
    }
    for (light, mut image) in &mut lamps {
        image.color.set_alpha(stroke.lamp_strength * light.0);
    }
    for (impact, mut node, mut visible, mut gradient) in &mut rings {
        let Some((centre, size, alpha)) = stroke.ripple(impact.0, clock.elapsed) else {
            *visible = Visibility::Hidden;
            continue;
        };
        *visible = Visibility::Inherited;
        *node = bounds(centre, size);
        if let [Gradient::Radial(ring)] = gradient.0.as_mut_slice() {
            for (stop, (_, strength)) in ring.stops.iter_mut().zip(RINGS) {
                stop.color = Color::srgba(0.69, 0.82, 0.84, alpha * strength);
            }
        }
    }
}
