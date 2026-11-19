//! The first crossing, shown once before the main menu.

use super::{LOGO, Opening, TitleBackground, spawn_fade};
use crate::{data::asset_exists, day_cycle::Phase, i18n::Lang, save::Settings, ui::UiFont};
use bevy::prelude::*;
use bevy::render::{
    ExtractSchedule, MainWorld, RenderApp, render_asset::RenderAssets,
    render_resource::PipelineCache, texture::GpuImage,
};

mod ferry;
mod story;
mod stroke;

const HARBOR: &str = "sprites/title/intro_harbor.png";
const TITLE_AT: f32 = 2.8;
const TITLE_FADE: f32 = 1.7;
const END_FADE: f32 = 0.75;
const READY_FRAMES: u8 = 2;
const STALL_SECS: f32 = 1.0;

#[derive(Component)]
pub(super) struct Crossing {
    elapsed: f32,
    started: bool,
    soundtrack: story::Soundtrack,
}

#[derive(Component)]
pub(super) struct Title;
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

pub(super) fn install(app: &mut App) {
    app.init_resource::<Opening>()
        .add_systems(Startup, spawn)
        .add_systems(
            Update,
            (animate, (ferry::animate, story::update))
                .chain()
                .run_if(in_state(Phase::Menu)),
        );
    app.sub_app_mut(RenderApp)
        .add_systems(ExtractSchedule, check_uploads);
}

pub(super) fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    settings: Res<Settings>,
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
    ferry::spawn(
        &mut commands,
        root,
        picture(ferry::HULL),
        picture(ferry::CREW),
        asset_exists(ferry::GLOW).then(|| assets.load(ferry::GLOW)),
    );
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
    story::spawn(&mut commands, root, &font);
    // The double chevron marks the skip action in every language.
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
    commands.entity(root).insert(Crossing {
        elapsed: 0.,
        started: false,
        soundtrack: story::Soundtrack::load(&assets, &lang.code, &settings),
    });
}

/// Start counting once the harbour can be drawn, including on a cold shader cache.
pub(super) fn check_uploads(
    mut world: ResMut<MainWorld>,
    images: Res<RenderAssets<GpuImage>>,
    pipelines: Res<PipelineCache>,
) {
    let Some(opening) = world.get_resource::<Opening>() else {
        return;
    };
    if opening.rendered_frames >= READY_FRAMES {
        return;
    }
    let assets = world.resource::<AssetServer>();
    let ready = pipelines.pipelines().next().is_some()
        && pipelines.waiting_pipelines().next().is_none()
        && opening.pictures.iter().all(|image| {
            images.get(image).is_some()
                || assets
                    .get_load_state(image.id())
                    .is_some_and(|s| s.is_failed())
        });
    let frames = if ready {
        opening.rendered_frames + 1
    } else {
        0
    };
    world.resource_mut::<Opening>().rendered_frames = frames;
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

pub(super) fn animate(
    mut commands: Commands,
    time: Res<Time<Real>>,
    assets: Res<AssetServer>,
    settings: Res<Settings>,
    opening: Option<Res<Opening>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut crossings: Query<(Entity, &mut Crossing)>,
    mut titles: Query<(Option<&mut ImageNode>, Option<&mut TextColor>), With<Title>>,
    mut curtains: Query<&mut BackgroundColor, With<Curtain>>,
) {
    let Ok((root, mut crossing)) = crossings.single_mut() else {
        return;
    };
    let skip = mouse.just_released(MouseButton::Left)
        || touches.any_just_released()
        || keys.get_just_released().next().is_some();
    let held = mouse.pressed(MouseButton::Left) || touches.iter().next().is_some();
    if skip || (crossing.elapsed >= stroke::DURATION && !held) {
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
    if opening.is_none_or(|loading| loading.rendered_frames < READY_FRAMES) {
        return;
    }
    if !crossing.started {
        if crossing.soundtrack.ready(&assets) {
            crossing
                .soundtrack
                .play_from(&mut commands, root, &assets, &settings, 0.);
            crossing.started = true;
        }
        return;
    }
    // The audio device can keep running while the window is suspended. Resume
    // both stems at the visible playhead so a stall never swallows a sentence.
    let dt = time.delta_secs();
    if dt > STALL_SECS {
        crossing.elapsed += 0.25;
        let elapsed = crossing.elapsed.min(stroke::DURATION);
        crossing
            .soundtrack
            .play_from(&mut commands, root, &assets, &settings, elapsed);
    } else {
        crossing.elapsed += dt;
    }
    let t = crossing.elapsed;
    for (image, text) in &mut titles {
        let alpha = ease((t - TITLE_AT) / TITLE_FADE);
        if let Some(mut image) = image {
            image.color.set_alpha(alpha);
        }
        if let Some(mut text) = text {
            text.0.set_alpha(alpha);
        }
    }
    for mut curtain in &mut curtains {
        curtain.0.set_alpha(
            (0.42 * (1. - ease(t / 0.40)))
                .max(ease((t - (stroke::DURATION - END_FADE)) / END_FADE)),
        );
    }
}
