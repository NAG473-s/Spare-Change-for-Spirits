//! Punting in the 768 × 256 illustration's coordinates, with +Y down.
use super::ease;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

pub(super) const CANVAS: Vec2 = Vec2::new(768., 256.);
pub(super) const WATER: f32 = 239.;
pub(super) const COUNT: u8 = 4;
const LEAD: f32 = 0.6;
const PERIOD: f32 = 3.;
const COAST: f32 = 1.4;
pub(super) const DURATION: f32 = LEAD + COUNT as f32 * PERIOD + COAST;

const DRIVE_END: f32 = 0.63;
const STRIDE: f32 = 201.;
const GLIDE: f32 = 0.57;
const PLANT_X: f32 = 204.;
const FEET: Vec2 = Vec2::new(96., 172.);
const HOOK: Vec2 = Vec2::new(702., 33.);
const SKIFF_PIVOT: Vec2 = Vec2::new(0.5, WATER / CANVAS.y);
const STAFF_SIZE: Vec2 = Vec2::new(224., 8.4);
const STAFF_TIP: Vec2 = Vec2::new(0.006, 0.5);
const SHOULDER: Vec2 = Vec2::new(0.13, 0.32);
const FIST: Vec2 = Vec2::new(0.90, 0.46);

#[derive(Component, Clone, Copy)]
pub(super) enum Part {
    Hull,
    Body,
    Staff,
    FarArm,
    NearArm,
    Lamp,
}

#[derive(Clone, Copy)]
pub(super) struct Pose {
    pub centre: Vec2,
    pub size: Vec2,
    pub angle: Rot2,
}

impl Pose {
    /// Position a painted pivot (fractions of the cutout) at a point on the boat.
    fn anchored(at: Vec2, pivot: Vec2, size: Vec2, angle: Rot2) -> Self {
        Self {
            centre: at + angle * ((Vec2::splat(0.5) - pivot) * size),
            size,
            angle,
        }
    }

    fn point(self, uv: Vec2) -> Vec2 {
        self.centre + self.angle * ((uv - Vec2::splat(0.5)) * self.size)
    }

    /// Keep the painted sleeve's thickness while its shoulder and fist meet both joints.
    fn sleeve(shoulder: Vec2, hand: Vec2, height: f32) -> Self {
        let reach = hand - shoulder;
        let rise = (FIST.y - SHOULDER.y) * height;
        let run = (reach.length_squared() - rise * rise).max(1.).sqrt();
        let width = run / (FIST.x - SHOULDER.x);
        let angle = Rot2::radians(reach.to_angle() - rise.atan2(run));
        Self::anchored(shoulder, SHOULDER, Vec2::new(width, height), angle)
    }
}

pub(super) struct Stroke {
    pub travel: f32,
    pub lamp_strength: f32,
    hull: Pose,
    body: Pose,
    staff: Pose,
    far_arm: Pose,
    near_arm: Pose,
    lamp: Pose,
}

impl Stroke {
    pub fn at(t: f32) -> Self {
        let cycles = ((t - LEAD) / PERIOD).clamp(0., f32::from(COUNT));
        let index = (cycles.floor() as u8).min(COUNT - 1);
        let phase = cycles - f32::from(index);
        let recovery = ((phase - DRIVE_END) / (1. - DRIVE_END)).clamp(0., 1.);
        let forward = ease(recovery);
        let push = ease(phase / DRIVE_END) - forward;
        let stow = if index == COUNT - 1 { forward } else { 0. };
        let intro_lift = 1. - ease(t / LEAD);

        // Cruise between strokes, accelerate under load, then coast past the lighthouse.
        let in_stroke = STRIDE * (GLIDE * phase + (1. - GLIDE) * ease(phase / 0.69));
        let glide_speed = STRIDE * GLIDE / PERIOD;
        let coast = (t - (DURATION - COAST)).clamp(0., COAST);
        let travel = f32::from(index) * STRIDE
            + in_stroke
            + glide_speed * ((t - LEAD).min(0.) + coast * (1. - 0.38 * coast / COAST));

        // A planted tip stays at one point in the harbour as the hull passes it.
        // During recovery it clears the water and reaches the next planting point.
        let lift = 32. * (PI * recovery).sin().powi(2) + 30. * stow + 22. * intro_lift;
        let tip = Vec2::new(
            PLANT_X - in_stroke + STRIDE * forward - 43. * stow,
            WATER - lift,
        );
        let rake = (24. - 56. * push - 13. * stow - 5. * intro_lift).to_radians();
        let down = Vec2::new(rake.sin(), rake.cos());
        let lower = tip - down * (146. - 0.76 * lift);
        let upper = lower - down * 29.;
        let staff = Pose::anchored(
            tip,
            STAFF_TIP,
            STAFF_SIZE,
            Rot2::radians((-down).to_angle()),
        );

        let heave = 1.7 * (t * 2.25).sin() + 1.1 * push;
        let roll = Rot2::degrees(0.44 * (t * 1.56).sin() + 0.38 * push);
        let hull = Pose::anchored(
            SKIFF_PIVOT * CANVAS + Vec2::Y * heave,
            SKIFF_PIVOT,
            CANVAS,
            roll,
        );
        let body = Pose::anchored(
            hull.point(FEET / CANVAS),
            Vec2::new(0.50, 0.961),
            Vec2::new(99., 144. - 3. * push),
            roll * Rot2::degrees(7. - 14. * push - 5. * stow),
        );
        let far_arm = Pose::sleeve(body.point(Vec2::new(0.58, 0.44)), lower, 21.);
        let near_arm = Pose::sleeve(body.point(Vec2::new(0.45, 0.42)), upper, 23.);
        let swing = 6.4 * (TAU * ((t - LEAD) / PERIOD - 0.13)).sin() + 2.0 * (1.56 * t).sin();
        let lamp = Pose::anchored(
            hull.point(HOOK / CANVAS),
            Vec2::new(0.52, 0.04),
            Vec2::new(42., 57.),
            Rot2::degrees(swing),
        );
        Self {
            travel,
            hull,
            body,
            staff,
            far_arm,
            near_arm,
            lamp,
            lamp_strength: 0.35 + 0.07 * (t * 3.8).sin(),
        }
    }

    pub fn pose(&self, part: Part) -> Pose {
        match part {
            Part::Hull => self.hull,
            Part::Body => self.body,
            Part::Staff => self.staff,
            Part::FarArm => self.far_arm,
            Part::NearArm => self.near_arm,
            Part::Lamp => self.lamp,
        }
    }

    /// Ring position stays in the water; its parent follows the ferry horizontally.
    pub fn ripple(&self, index: u8, t: f32) -> Option<(Vec2, Vec2, f32)> {
        const LIFE: f32 = 2.35;
        let age = (t - LEAD - f32::from(index) * PERIOD) / LIFE;
        if !(0.0..1.0).contains(&age) {
            return None;
        }
        let centre = Vec2::new(
            PLANT_X + f32::from(index) * STRIDE - self.travel,
            WATER + 0.8,
        );
        let width = 12. + 86. * (1. - (1. - age).powi(2));
        let alpha = 0.56 * (1. - ease(age)) * ease(age * 16.);
        Some((centre, Vec2::new(width, 3. + width * 0.15), alpha))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_planted_staff_meets_its_ripple_as_the_ferry_advances() {
        for index in 0..COUNT {
            for phase in [0.02, 0.18, 0.40, 0.61] {
                let t = LEAD + (f32::from(index) + phase) * PERIOD;
                let s = Stroke::at(t);
                let tip = s.staff.point(STAFF_TIP);
                let (ring, _, _) = s.ripple(index, t).unwrap();
                assert!((tip.x - ring.x).abs() < 0.01);
                assert!((s.travel + tip.x - (PLANT_X + f32::from(index) * STRIDE)).abs() < 0.01);
                assert!((tip.y - WATER).abs() < 0.01);
            }
        }
        assert!(Stroke::at(LEAD + 0.82 * PERIOD).staff.point(STAFF_TIP).y < WATER - 25.);
        assert!(Stroke::at(DURATION).staff.point(STAFF_TIP).y < WATER - 25.);
    }

    #[test]
    fn both_hands_grip_the_staff_through_each_stroke_and_the_final_coast() {
        let mut previous_x = f32::NEG_INFINITY;
        for frame in 0..=(DURATION * 60.) as u32 {
            let stroke = Stroke::at(frame as f32 / 60.);
            assert!(stroke.travel >= previous_x);
            previous_x = stroke.travel;
            let tip = stroke.staff.point(STAFF_TIP);
            let along = stroke.staff.angle * Vec2::X;
            for hand in [stroke.far_arm.point(FIST), stroke.near_arm.point(FIST)] {
                assert!(along.perp_dot(hand - tip).abs() < 0.01);
                assert!((95.0..195.0).contains(&along.dot(hand - tip)));
            }
        }
    }
}
