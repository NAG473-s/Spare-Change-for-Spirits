//! Keep the outer stalls in the part of the market left visible by the trader list.
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::{FigureKind, MARKET, STALL_HALF_FOOTPRINT, stall_transform};
use crate::data::{GameData, VendorDef};
use crate::harbor::Location;
use crate::scene::{CAMERA_POS, CAMERA_TARGET, MainCamera};
use crate::ui::PANEL_WIDTH;

const HEIGHT: f32 = 2.50;
const TRADER_HALF_WIDTH: f32 = 0.35;
const EDGE_MARGIN: f32 = 12.0;

fn camera_offset(vendors: &[VendorDef], window: Vec2, sidebar: f32, fov: f32) -> Vec3 {
    if vendors.is_empty() || window.y <= 0.0 || window.x - sidebar <= 2.0 * EDGE_MARGIN {
        return Vec3::ZERO;
    }
    // Side edges of the visible play area as slopes from the camera in view space.
    let slope_per_pixel = 2.0 * (fov / 2.0).tan() / window.y;
    let left = (EDGE_MARGIN - window.x / 2.0) * slope_per_pixel;
    let right = (window.x / 2.0 - sidebar - EDGE_MARGIN) * slope_per_pixel;
    let forward = (CAMERA_TARGET - CAMERA_POS).normalize();
    let across = forward.cross(Vec3::Y).normalize();

    // Each corner limits how far the camera can pan at its original distance.
    let mut min_pan = f32::NEG_INFINITY;
    let mut max_pan = f32::INFINITY;
    for vendor in vendors {
        let at = stall_transform(vendor);
        let figure = FigureKind::for_vendor(&vendor.id).spot();
        let min_x = (-STALL_HALF_FOOTPRINT.x).min(figure.x - TRADER_HALF_WIDTH);
        let max_x = STALL_HALF_FOOTPRINT.x.max(figure.x + TRADER_HALF_WIDTH);
        for x in [min_x, max_x] {
            for y in [0.0, HEIGHT] {
                for z in [-STALL_HALF_FOOTPRINT.y, STALL_HALF_FOOTPRINT.y] {
                    let point = at.transform_point(Vec3::new(x, y, z)) - CAMERA_POS;
                    let depth = point.dot(forward);
                    let side = point.dot(across);
                    min_pan = min_pan.max(side - right * depth);
                    max_pan = max_pan.min(side - left * depth);
                }
            }
        }
    }
    // Back up only far enough for both limits to fit, then take the smallest pan.
    let retreat = ((min_pan - max_pan) / (right - left)).max(0.0);
    let min_pan = min_pan - right * retreat;
    let max_pan = max_pan - left * retreat;
    let pan = min_pan.max(0.0) + max_pan.min(0.0);
    across * pan - forward * retreat
}

pub(crate) fn fit_stalls(
    window: Single<Ref<Window>, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    location: Res<Location>,
    data: Res<GameData>,
    mut camera: Single<(&Projection, &mut Transform), With<MainCamera>>,
) {
    if !location.is_changed() && !window.is_changed() && !scale.is_changed() {
        return;
    }
    let (projection, at) = &mut *camera;
    if let Projection::Perspective(lens) = *projection {
        let shift = camera_offset(
            &data.vendors,
            window.size(),
            PANEL_WIDTH * scale.0,
            lens.fov,
        );
        at.translation = MARKET + CAMERA_POS + shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::TABLE;
    use crate::scene::DIORAMA_FOV;

    #[test]
    fn traders_and_counter_ends_fit_beside_the_list_as_the_window_narrows() {
        let data = GameData::load();
        for (width, height, ui_size) in [
            (1280., 720., 1.),
            (1280., 800., 1.),
            (1024., 768., 1.),
            (1280., 720., 1.5),
        ] {
            let window = Vec2::new(width, height);
            let sidebar = PANEL_WIDTH * height / 720. * ui_size;
            let shift = camera_offset(&data.vendors, window, sidebar, DIORAMA_FOV);
            let camera = Transform::from_translation(CAMERA_POS + shift)
                .looking_at(CAMERA_TARGET + shift, Vec3::Y);
            for vendor in &data.vendors {
                let pitch = stall_transform(vendor);
                let operator = FigureKind::for_vendor(&vendor.id).spot();
                for local in [
                    Vec3::new(-TABLE.x / 2., TABLE.y, TABLE.z / 2.),
                    Vec3::new(TABLE.x / 2., TABLE.y, TABLE.z / 2.),
                    operator + Vec3::Y * 1.4,
                ] {
                    let p = camera
                        .compute_affine()
                        .inverse()
                        .transform_point3(pitch.transform_point(local));
                    let x = width * 0.5 + p.x / -p.z * height * 0.5 / (DIORAMA_FOV * 0.5).tan();
                    assert!(
                        (EDGE_MARGIN - 0.01..=width - sidebar - EDGE_MARGIN + 0.01).contains(&x),
                        "{} at {window:?}, UI size {ui_size}: x = {x}",
                        vendor.id
                    );
                }
            }
            if width == 1280. && height == 720. && ui_size == 1. {
                assert_eq!(shift, Vec3::ZERO);
            }
        }
    }
}
