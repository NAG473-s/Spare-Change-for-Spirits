//! The first crossing, shown once before the language picker.

use super::{LOGO, Opening, TitleBackground, spawn_fade};
use crate::{data::asset_exists, day_cycle::Phase, i18n::Lang, ui::UiFont};
use bevy::prelude::*;
use bevy::render::{MainWorld, render_asset::RenderAssets, texture::GpuImage};

const HARBOR: &str = "sprites/title/intro_harbor.png";
const FERRY: &str = "sprites/title/intro_ferry.png";
const GLOW: &str = "sprites/fx/glow.png";
const DURATION: f32 = 4.8;
const BOAT_WIDTH: f32 = 38.0;
const WATERLINE: f32 = 78.0;

#[derive(Component, Default)]
pub(super) struct Crossing {
    elapsed: f32,
}

#[derive(Component)]
pub(super) struct Ferry;
#[derive(Component)]
pub(super) struct Title;
#[derive(Component)]
pub(super) struct Lantern;
#[derive(Component)]
pub(super) struct Curtain;

fn rect(x: f32, y: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(x),
        top: percent(y),
        width: percent(width),
        height: percent(height),
        ..default()
    }
}

pub(super) fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    mut opening: ResMut<Opening>,
) {
    let mut picture = |path: &'static str| {
        asset_exists(path).then(|| {
            let image: Handle<Image> = assets.load(path);
            opening.pictures.push(image.clone());
            image
        })
    };
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..rect(0., 0., 100., 100.)
            },
            BackgroundColor(Color::srgb(0.07, 0.07, 0.14)),
            GlobalZIndex(300),
            Pickable::default(),
            DespawnOnExit(Phase::Menu),
        ))
        .id();
    if let Some(image) = picture(HARBOR) {
        commands.spawn((
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
            Pickable::IGNORE,
            ChildOf(root),
        ));
    } else {
        commands.spawn((
            rect(0., 61., 100., 39.),
            BackgroundColor(Color::srgb(0.08, 0.18, 0.23)),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    let ferry = commands
        .spawn((
            Ferry,
            Node {
                position_type: PositionType::Absolute,
                left: percent(6),
                top: percent(WATERLINE),
                width: percent(BOAT_WIDTH),
                max_width: vh(76),
                aspect_ratio: Some(3.0),
                ..default()
            },
            // Hull and its reflection share the same waterline at any window size.
            UiTransform::from_translation(Val2::percent(0., -92.)),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    if let Some(image) = picture(FERRY) {
        commands.spawn((
            ImageNode {
                image: image.clone(),
                image_mode: NodeImageMode::Stretch,
                flip_y: true,
                color: Color::srgba(0.51, 0.64, 0.75, 0.17),
                ..default()
            },
            rect(0., 89., 100., 35.),
            Pickable::IGNORE,
            ChildOf(ferry),
        ));
        commands.spawn((
            ImageNode::new(image),
            rect(0., 0., 100., 100.),
            Pickable::IGNORE,
            ChildOf(ferry),
        ));
    } else {
        for (x, y, w, h, tint) in [
            (1., 70., 97., 22., Color::srgb(0.28, 0.19, 0.18)),
            (87., 18., 1., 54., Color::srgb(0.44, 0.32, 0.23)),
            (88., 26., 4., 13., Color::srgb(1.0, 0.80, 0.42)),
        ] {
            commands.spawn((
                Node {
                    border_radius: BorderRadius::all(percent(38)),
                    ..rect(x, y, w, h)
                },
                BackgroundColor(tint),
                Pickable::IGNORE,
                ChildOf(ferry),
            ));
        }
    }
    if asset_exists(GLOW) {
        commands.spawn((
            Lantern,
            ImageNode {
                image: assets.load(GLOW),
                color: Color::srgba(1., 0.75, 0.35, 0.39),
                ..default()
            },
            rect(81.6, 1., 18., 54.),
            Pickable::IGNORE,
            ChildOf(ferry),
        ));
    }
    let title = commands
        .spawn((
            Title,
            Node {
                position_type: PositionType::Absolute,
                top: percent(10),
                width: percent(34),
                max_width: vh(63),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    if let Some(image) = picture(LOGO) {
        commands.entity(title).insert(ImageNode {
            color: Color::WHITE.with_alpha(0.),
            ..ImageNode::new(image)
        });
    } else {
        commands.entity(title).insert((
            Text::new(lang.t("game.title")),
            TextFont {
                font: font.display.clone(),
                font_size: px(36).into(),
                ..default()
            },
            TextColor(Color::srgb(1., 0.86, 0.57).with_alpha(0.)),
        ));
    }
    // The familiar double chevron is visible even before choosing a language.
    for inset in [24., 32.] {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(inset),
                bottom: px(23),
                width: px(8),
                height: px(8),
                border: UiRect {
                    right: px(2),
                    top: px(2),
                    ..default()
                },
                ..default()
            },
            BorderColor::all(Color::srgba(0.96, 0.87, 0.69, 0.84)),
            UiTransform::from_rotation(Rot2::degrees(45.)),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    commands.spawn((
        Curtain,
        rect(0., 0., 100., 100.),
        BackgroundColor(Color::BLACK.with_alpha(0.42)),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    commands.entity(root).insert(Crossing::default());
}

/// Wait for the ferry and harbour to reach the renderer before timing their crossing.
pub(super) fn check_uploads(mut world: ResMut<MainWorld>, images: Res<RenderAssets<GpuImage>>) {
    let Some(opening) = world.get_resource::<Opening>() else {
        return;
    };
    if opening.rendered_frames >= 2 {
        return;
    }
    let assets = world.resource::<AssetServer>();
    if opening.pictures.iter().all(|image| {
        images.get(image).is_some()
            || assets
                .get_load_state(image.id())
                .is_some_and(|s| s.is_failed())
    }) {
        world.resource_mut::<Opening>().rendered_frames += 1;
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

pub(super) fn animate(
    mut commands: Commands,
    time: Res<Time<Real>>,
    opening: Option<Res<Opening>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut crossings: Query<(Entity, &mut Crossing)>,
    mut ferries: Query<&mut Node, With<Ferry>>,
    mut titles: Query<
        (Option<&mut ImageNode>, Option<&mut TextColor>),
        (With<Title>, Without<Lantern>),
    >,
    mut lanterns: Query<&mut ImageNode, With<Lantern>>,
    mut curtains: Query<&mut BackgroundColor, With<Curtain>>,
) {
    let Ok((root, mut crossing)) = crossings.single_mut() else {
        return;
    };
    let skip = mouse.just_released(MouseButton::Left)
        || touches.any_just_released()
        || keys.get_just_released().next().is_some();
    let held = mouse.pressed(MouseButton::Left) || touches.iter().next().is_some();
    if skip || (crossing.elapsed >= DURATION && !held) {
        commands.entity(root).despawn();
        commands.remove_resource::<Opening>();
        debug!(
            "First-launch crossing ended after {:.2}s of playback (skip={skip})",
            crossing.elapsed
        );
        if !skip {
            spawn_fade(&mut commands, 0.3);
        }
        return;
    }
    if opening.is_none_or(|loading| loading.rendered_frames < 2) {
        return;
    }
    // Shader compilation or resuming a suspended window must not consume the
    // crossing before the player has seen it. At normal frame rates this is real time.
    crossing.elapsed += time.delta_secs().min(0.25);
    let t = crossing.elapsed;
    let progress = (t / DURATION).min(1.);
    for mut node in &mut ferries {
        node.left = percent(6. + 43. * progress);
        node.top = percent(WATERLINE + 0.38 * (t * 2.7).sin());
    }
    for (image, text) in &mut titles {
        let alpha = ease((t - 0.80) / 1.15);
        if let Some(mut image) = image {
            image.color.set_alpha(alpha);
        }
        if let Some(mut text) = text {
            text.0.set_alpha(alpha);
        }
    }
    for mut image in &mut lanterns {
        image.color.set_alpha(0.36 + 0.08 * (t * 3.8).sin());
    }
    for mut curtain in &mut curtains {
        curtain
            .0
            .set_alpha((0.42 * (1. - ease(t / 0.40))).max(ease((t - (DURATION - 0.5)) / 0.5)));
    }
}
