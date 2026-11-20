//! Deform the painted silhouette around the head, hands and trailing hem.
//! Each ghost owns a small mesh. The portrait or its idle sheet still supplies
//! the drawing, and the hover volume and simulation stay on the parent.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use super::{GHOST_SIZE, Ghost, GhostState, PATIENCE_IMPATIENT};
use crate::save::Settings;

const COLS: usize = 12;
const ROWS: usize = 18;
const FRAME: f32 = 1.0 / 18.0;
const BLEND: f32 = 0.24;

pub(super) fn mesh() -> Mesh {
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut indices = Vec::new();
    for y in 0..=ROWS {
        for x in 0..=COLS {
            let p = Vec2::new(x as f32 / COLS as f32, y as f32 / ROWS as f32);
            pos.push(point(p));
            uv.push([p.x, 1.0 - p.y]);
            if x < COLS && y < ROWS {
                let i = (y * (COLS + 1) + x) as u32;
                let j = i + COLS as u32 + 1;
                indices.extend_from_slice(&[i, i + 1, j, j, i + 1, j + 1]);
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; pos.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(indices))
}

fn point(uv: Vec2) -> [f32; 3] {
    [(uv.x - 0.5) * GHOST_SIZE.x, uv.y * GHOST_SIZE.y, 0.0]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Act {
    Arrive,
    Browse,
    Approach,
    Idle,
    Impatient,
    Pay,
    Goodbye,
    Flee,
    Ascend,
}

impl Act {
    /// The activities that can have their own drawn sheet; the idle loop is
    /// `<id>_sheet.png`.
    pub(super) const DRAWN: [Act; 8] = [
        Act::Arrive,
        Act::Browse,
        Act::Approach,
        Act::Impatient,
        Act::Pay,
        Act::Goodbye,
        Act::Flee,
        Act::Ascend,
    ];

    /// File name part of the drawn sheet: `<id>_<file>.png`.
    pub(super) fn file(self) -> &'static str {
        match self {
            Act::Arrive => "arrive",
            Act::Browse => "browse",
            Act::Approach => "approach",
            Act::Idle => "sheet",
            Act::Impatient => "impatient",
            Act::Pay => "pay",
            Act::Goodbye => "goodbye",
            Act::Flee => "flee",
            Act::Ascend => "ascend",
        }
    }

    fn for_state(state: GhostState, patience: f32, patient: bool) -> Self {
        match state {
            GhostState::Entering => Self::Arrive,
            GhostState::Browsing => Self::Browse,
            GhostState::ToCounter => Self::Approach,
            GhostState::Waiting if !patient && patience < PATIENCE_IMPATIENT => Self::Impatient,
            GhostState::Waiting => Self::Idle,
            GhostState::AtRegister => Self::Pay,
            GhostState::Leaving => Self::Goodbye,
            GhostState::Fleeing => Self::Flee,
            GhostState::Ascending => Self::Ascend,
        }
    }

    fn pose(self, t: f32, phase: f32) -> Pose {
        let cycle = |speed: f32| (speed * t + phase).sin();
        let beat = |speed: f32| 0.5 - 0.5 * (speed * t).cos();
        match self {
            // Peek over the threshold and tuck one hand against the chest.
            Self::Arrive => Pose::new(
                0.09 * cycle(2.6),
                0.025,
                0.12 * cycle(2.6),
                -0.055,
                0.07,
                0.10 * cycle(3.1),
            ),
            // Lean close, look down at the shelf and reach out.
            Self::Browse => {
                let reach = beat(2.0);
                Pose::new(
                    0.065 * reach,
                    -0.065 * reach,
                    -0.19 * reach,
                    0.145 * reach,
                    0.036 * reach,
                    0.025 * cycle(2.0),
                )
            }
            // Glide forward with a long, trailing hem and an alternating arm stroke.
            Self::Approach => Pose::new(
                0.026,
                0.022 * cycle(3.4),
                -0.052,
                0.038 * cycle(3.4),
                0.060 * cycle(3.4),
                -0.105 + 0.035 * cycle(3.4),
            ),
            // Settle and look around without counting down in patient mode.
            Self::Idle => Pose::new(
                0.035 * cycle(1.65),
                0.014 * cycle(1.1),
                0.074 * cycle(1.65),
                -0.026,
                0.019 * cycle(1.25),
                0.045 * cycle(1.45),
            ),
            Self::Impatient => {
                let tap = beat(6.4).powi(3);
                Pose::new(
                    -0.025,
                    0.024 * tap,
                    0.12 * cycle(3.2),
                    0.067,
                    0.105 * tap,
                    -0.065 * tap,
                )
            }
            // Offer a coin, dip the head, and draw the hand back.
            Self::Pay => {
                let offer = beat(3.0).powi(2);
                Pose::new(
                    0.022,
                    -0.065 * offer,
                    -0.085 * offer,
                    0.105 * offer,
                    -0.060 * offer,
                    0.026 * cycle(1.9),
                )
            }
            Self::Goodbye => Pose::new(
                -0.050,
                0.020,
                0.13,
                -0.052 + 0.080 * cycle(5.4),
                0.115,
                0.074 * cycle(3.0),
            ),
            Self::Flee => Pose::new(
                -0.084,
                0.048,
                -0.16,
                -0.105,
                0.115 + 0.023 * cycle(7.0),
                0.158 * cycle(5.8),
            ),
            Self::Ascend => {
                let rise = (t * 2.3).min(1.0);
                Pose::new(
                    0.0,
                    0.071 * rise,
                    0.045 * cycle(2.0),
                    0.115 * rise,
                    0.130 * rise,
                    -0.10 * rise,
                )
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Pose {
    head: Vec2,
    tilt: f32,
    hand: Vec2,
    tail: f32,
}

impl Pose {
    fn new(x: f32, y: f32, tilt: f32, hx: f32, hy: f32, tail: f32) -> Self {
        Self {
            head: Vec2::new(x, y),
            tilt,
            hand: Vec2::new(hx, hy),
            tail,
        }
    }

    fn blend(self, to: Self, k: f32) -> Self {
        Self {
            head: self.head.lerp(to.head, k),
            tilt: self.tilt.lerp(to.tilt, k),
            hand: self.hand.lerp(to.hand, k),
            tail: self.tail.lerp(to.tail, k),
        }
    }
}

/// Coordinates are measured from the bottom left of each particular portrait.
#[derive(Component)]
pub(super) struct Rig {
    head: Vec2,
    hand: Vec2,
    secondary: Vec2,
    tempo: f32,
    act: Act,
    elapsed: f32,
    next_frame: f32,
    previous: Pose,
    current: Pose,
    /// Activities with a drawn sheet of their own: the drawing carries the
    /// movement, so the mesh stays still for them.
    drawn: Vec<Act>,
}

/// Where the drawn sheet of a ghost's activity lives.
pub(super) fn gesture_path(id: &str, act: Act) -> String {
    format!("sprites/ghosts/{id}_{}.png", act.file())
}

impl Rig {
    pub(super) fn new(id: &str, voice: f32) -> Self {
        let (head, hand, secondary) = match id {
            "cat" => ((0.48, 0.72), (0.33, 0.48), (0.71, 0.88)),
            "bride" => ((0.48, 0.78), (0.65, 0.54), (0.30, 0.45)),
            "child" => ((0.40, 0.60), (0.82, 0.52), (0.29, 0.41)),
            "edwin" => ((0.49, 0.77), (0.62, 0.53), (0.40, 0.47)),
            "forgetful" => ((0.54, 0.75), (0.81, 0.50), (0.43, 0.54)),
            "hedda" => ((0.49, 0.72), (0.61, 0.43), (0.47, 0.42)),
            "ida" => ((0.47, 0.75), (0.68, 0.54), (0.51, 0.34)),
            "knight" => ((0.39, 0.75), (0.75, 0.46), (0.21, 0.36)),
            "poltergeist" => ((0.55, 0.69), (0.82, 0.39), (0.22, 0.53)),
            "sailor" => ((0.49, 0.69), (0.62, 0.45), (0.39, 0.46)),
            "shy" => ((0.47, 0.69), (0.43, 0.55), (0.69, 0.40)),
            "vain" => ((0.45, 0.74), (0.79, 0.64), (0.38, 0.50)),
            _ => ((0.5, 0.75), (0.7, 0.48), (0.32, 0.46)),
        };
        Self {
            head: head.into(),
            hand: hand.into(),
            secondary: secondary.into(),
            tempo: voice.clamp(0.78, 1.28),
            act: Act::Arrive,
            elapsed: 0.0,
            next_frame: 0.0,
            previous: default(),
            current: default(),
            drawn: Act::DRAWN
                .into_iter()
                .filter(|a| crate::data::asset_exists(&gesture_path(id, *a)))
                .collect(),
        }
    }

    /// The current activity, if it has a drawn sheet.
    pub(super) fn drawn_act(&self) -> Option<Act> {
        self.drawn.contains(&self.act).then_some(self.act)
    }

    fn deform(&self, p: Vec2) -> Vec2 {
        let weight = |centre: Vec2, radius: Vec2| {
            (1.0 - ((p - centre) / radius).length_squared())
                .max(0.0)
                .powi(2)
        };
        let pose = self.current;
        let head_weight = weight(self.head, Vec2::new(0.53, 0.39));
        let arm = weight(self.hand, Vec2::new(0.37, 0.30));
        let other = weight(self.secondary, Vec2::new(0.29, 0.26));
        let neck = self.head - Vec2::Y * 0.15;
        let d = p - neck;
        let tilt = Vec2::new(-d.y, d.x) * pose.tilt;
        let trail = (1.0 - p.y / 0.50).clamp(0.0, 1.0).powi(2);
        p + (pose.head + tilt) * head_weight
            + pose.hand * arm
            + Vec2::new(-pose.hand.x * 0.44, pose.hand.y * 0.64) * other
            + Vec2::new(pose.tail, pose.hand.y * 0.22) * trail
    }
}

pub(super) fn animate(
    time: Res<Time>,
    settings: Res<Settings>,
    ghosts: Query<&Ghost>,
    mut bodies: Query<(&ChildOf, &Mesh3d, &mut Rig)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs();
    for (child, mesh, mut rig) in &mut bodies {
        let Ok(ghost) = ghosts.get(child.parent()) else {
            continue;
        };
        let act = Act::for_state(
            ghost.state,
            ghost.patience / ghost.max_patience.max(0.01),
            settings.patient_ghosts,
        );
        if act != rig.act {
            rig.previous = rig.current;
            rig.act = act;
            rig.elapsed = 0.0;
            rig.next_frame = 0.0;
        }
        rig.elapsed += dt;
        if rig.elapsed < rig.next_frame {
            continue;
        }
        rig.next_frame = rig.elapsed + FRAME;
        let k = (rig.elapsed / BLEND).clamp(0.0, 1.0);
        let pose = if rig.drawn.contains(&act) {
            Pose::default()
        } else {
            act.pose(rig.elapsed * rig.tempo, ghost.bob_offset)
        };
        rig.current = rig.previous.blend(pose, k * k * (3.0 - 2.0 * k));
        let Some(mut surface) = meshes.get_mut(mesh) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x3(vertices)) =
            surface.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        for (i, vertex) in vertices.iter_mut().enumerate() {
            let p = Vec2::new(
                (i % (COLS + 1)) as f32 / COLS as f32,
                (i / (COLS + 1)) as f32 / ROWS as f32,
            );
            *vertex = point(rig.deform(p));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_know_every_drawn_gesture() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for tool in ["tools/prepare_sprite.py", "tools/check_models.py"] {
            let text = std::fs::read_to_string(root.join(tool)).unwrap();
            let line = text.lines().find(|l| l.starts_with("GESTURES = ")).unwrap();
            for act in Act::DRAWN {
                assert!(
                    line.contains(&format!("\"{}\"", act.file())),
                    "{tool}: {act:?}"
                );
            }
            assert_eq!(line.matches('"').count(), Act::DRAWN.len() * 2, "{tool}");
        }
    }
    #[test]
    fn each_activity_moves_the_painted_mesh_differently() {
        let actions = [
            Act::Arrive,
            Act::Browse,
            Act::Approach,
            Act::Idle,
            Act::Impatient,
            Act::Pay,
            Act::Goodbye,
            Act::Flee,
            Act::Ascend,
        ];
        let mut rig = Rig::new("cat", 1.0);
        let mut samples = Vec::new();
        for act in actions {
            rig.current = act.pose(0.92, 0.4);
            let samples_now = [
                rig.deform(rig.hand),
                rig.deform(rig.head),
                rig.deform(Vec2::new(0.52, 0.07)),
            ];
            assert!(!samples.contains(&samples_now));
            samples.push(samples_now);
        }
        assert_eq!(Act::for_state(GhostState::Waiting, 0.1, true), Act::Idle);
        assert_eq!(
            Act::for_state(GhostState::Waiting, 0.1, false),
            Act::Impatient
        );
    }
}
