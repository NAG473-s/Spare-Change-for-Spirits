//! Full-screen illustrated films at the end of a ghost's last chapter.
mod media;

use bevy::{
    audio::Volume,
    prelude::*,
    tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures::check_ready},
    text::LineBreak,
};
use std::{collections::VecDeque, sync::Arc};

use crate::{
    data::{GameData, asset_exists, assets_dir},
    i18n::Lang,
    save::Settings,
    ui::UiFont,
};
use media::Movie;

/// The story ghost's stable data id, e.g. `"edwin"`.
#[derive(Message)]
pub(crate) struct PlayStoryFilm(pub String);

pub struct StoryFilmPlugin;

impl Plugin for StoryFilmPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayStoryFilm>()
            .init_resource::<FilmQueue>()
            // Chapter completions are sent in Update. Input picking has already
            // finished when a released skip removes the cover in PostUpdate.
            .add_systems(PostUpdate, (enqueue, advance).chain());
    }
}

#[derive(Default, Resource)]
pub(crate) struct FilmQueue {
    pending: VecDeque<String>,
    current: Option<Screen>,
    previously_paused: bool,
    release_guard: Option<Entity>,
}

pub(crate) fn outside_film(queue: Option<Res<FilmQueue>>) -> bool {
    queue.is_none_or(|q| q.current.is_none())
}

fn enqueue(
    mut messages: MessageReader<PlayStoryFilm>,
    data: Res<GameData>,
    mut films: ResMut<FilmQueue>,
) {
    for PlayStoryFilm(id) in messages.read() {
        if data.stories.iter().any(|story| &story.ghost == id) {
            films.pending.push_back(id.clone());
        }
    }
}

struct Screen {
    root: Entity,
    artwork: Handle<Image>,
    caption: Entity,
    opening: Option<Task<Result<(Arc<Movie>, Image), String>>>,
    movie: Option<Arc<Movie>>,
    decoding: Option<Task<Result<Image, String>>>,
    requested_frame: usize,
    voice: Option<Handle<AudioSource>>,
    speaker: Option<Entity>,
    duration: Option<f32>,
    elapsed: f32,
    armed: bool,
}

impl Screen {
    fn spawn(
        commands: &mut Commands,
        ghost: &str,
        server: &AssetServer,
        images: &mut Assets<Image>,
        lang: &Lang,
        font: &UiFont,
    ) -> Self {
        let root = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::BLACK),
                GlobalZIndex(1100),
                Pickable::default(),
            ))
            .id();
        let artwork = images.add(Image {
            data: Some(vec![0, 0, 0, 255]),
            ..Image::default()
        });
        let path = format!("videos/endings/{ghost}.avi");
        let opening = asset_exists(&path).then(|| {
            let path = assets_dir().join(path);
            IoTaskPool::get().spawn(async move { Movie::load(&path) })
        });
        // A missing film still has a caption, the localized voice and, where
        // available, the ghost's existing portrait.
        let portrait = format!("sprites/ghosts/{ghost}.png");
        if opening.is_some() || asset_exists(&portrait) {
            let (handle, aspect) = if opening.is_some() {
                (artwork.clone(), 16. / 9.)
            } else {
                (server.load(portrait), 2. / 3.)
            };
            commands.spawn((
                ImageNode::new(handle).with_mode(NodeImageMode::Stretch),
                Node {
                    width: percent(100),
                    max_width: vh(100. * aspect),
                    aspect_ratio: Some(aspect),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        let caption = commands
            .spawn((
                Text::new(lang.t(&format!("story.{ghost}.ending"))),
                TextFont {
                    font: font.regular.clone(),
                    font_size: px(21).into(),
                    ..default()
                },
                TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
                TextColor(Color::srgb(1., 0.97, 0.88)),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: percent(5),
                    max_width: percent(86),
                    padding: UiRect::axes(px(18), px(8)),
                    border_radius: BorderRadius::all(px(5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.045, 0.035, 0.06, 0.9)),
                Visibility::Hidden,
                Pickable::IGNORE,
                ChildOf(root),
            ))
            .id();
        // The same skip chevrons as the opening crossing, drawn without a font.
        for inset in [24., 32.] {
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: px(inset),
                    top: px(23),
                    width: px(8),
                    height: px(8),
                    border: UiRect {
                        right: px(2),
                        top: px(2),
                        ..default()
                    },
                    ..default()
                },
                BorderColor::all(Color::srgb(0.96, 0.88, 0.74)),
                UiTransform::from_rotation(Rot2::degrees(45.)),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
        let voice = format!("sounds/endings/{ghost}/{}.ogg", lang.code);
        Self {
            root,
            artwork,
            caption,
            opening,
            movie: None,
            decoding: None,
            requested_frame: 0,
            voice: asset_exists(&voice).then(|| server.load(voice)),
            speaker: None,
            duration: None,
            elapsed: 0.,
            armed: false,
        }
    }

    fn prepare(
        &mut self,
        commands: &mut Commands,
        server: &AssetServer,
        images: &mut Assets<Image>,
        sounds: &Assets<AudioSource>,
        settings: &Settings,
    ) {
        if let Some(task) = &mut self.opening {
            let Some(result) = check_ready(task) else {
                return;
            };
            self.opening = None;
            match result {
                Ok((movie, first)) => {
                    if let Some(mut image) = images.get_mut(&self.artwork) {
                        *image = first;
                    }
                    self.movie = Some(movie);
                }
                Err(err) => warn!("Unable to play story film: {err}"),
            }
        }
        let mut seconds = None;
        if let Some(clip) = &self.voice {
            if let Some(sound) = sounds.get(clip) {
                seconds = media::narration_seconds(&sound.bytes);
                self.speaker = Some(
                    commands
                        .spawn((
                            AudioPlayer::new(clip.clone()),
                            PlaybackSettings {
                                volume: narration_volume(settings),
                                ..PlaybackSettings::ONCE
                            },
                            ChildOf(self.root),
                        ))
                        .id(),
                );
            } else if !server
                .get_load_state(clip.id())
                .is_some_and(|s| s.is_failed())
            {
                return;
            }
        }
        commands.entity(self.caption).insert(Visibility::Inherited);
        self.duration =
            Some(seconds.unwrap_or_else(|| self.movie.as_ref().map_or(7., |m| m.duration)));
    }

    fn show_frame(&mut self, fraction: f32, images: &mut Assets<Image>) {
        if let Some(task) = &mut self.decoding {
            let Some(result) = check_ready(task) else {
                return;
            };
            self.decoding = None;
            match result {
                Ok(decoded) => {
                    if let Some(mut image) = images.get_mut(&self.artwork) {
                        *image = decoded;
                    }
                }
                Err(err) => {
                    warn!("Unable to decode story film frame: {err}");
                    self.movie = None;
                }
            }
        }
        let Some(movie) = &self.movie else { return };
        let index = movie.frame_at(fraction);
        if index != self.requested_frame {
            self.requested_frame = index;
            let movie = Arc::clone(movie);
            self.decoding =
                Some(AsyncComputeTaskPool::get().spawn(async move { movie.decode(index) }));
        }
    }
}

fn narration_volume(settings: &Settings) -> Volume {
    Volume::Linear(if settings.sound {
        settings.narration
    } else {
        0.
    })
}

fn advance(
    mut commands: Commands,
    mut films: ResMut<FilmQueue>,
    real: Res<Time<Real>>,
    mut clock: ResMut<Time<Virtual>>,
    server: Res<AssetServer>,
    sounds: Res<Assets<AudioSource>>,
    mut images: ResMut<Assets<Image>>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    settings: Res<Settings>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    pads: Query<&Gamepad>,
    mut sinks: Query<&mut AudioSink>,
) {
    let pointer_held = mouse.pressed(MouseButton::Left)
        || mouse.pressed(MouseButton::Right)
        || touches.iter().next().is_some()
        || pads
            .iter()
            .any(|pad| pad.pressed(GamepadButton::South) || pad.pressed(GamepadButton::East));
    if !pointer_held && let Some(guard) = films.release_guard.take() {
        commands.entity(guard).despawn();
    }
    if films.current.is_none() {
        let Some(ghost) = films.pending.pop_front() else {
            return;
        };
        films.previously_paused = clock.is_paused();
        clock.pause();
        films.current = Some(Screen::spawn(
            &mut commands,
            &ghost,
            &server,
            &mut images,
            &lang,
            &font,
        ));
        return;
    }
    let screen = films.current.as_mut().unwrap();
    let buttons = [
        GamepadButton::South,
        GamepadButton::East,
        GamepadButton::Start,
    ];
    let skip_keys = [KeyCode::Escape, KeyCode::Enter, KeyCode::Space];
    let held = mouse.pressed(MouseButton::Left)
        || touches.iter().next().is_some()
        || skip_keys.iter().any(|k| keys.pressed(*k))
        || pads
            .iter()
            .any(|pad| buttons.iter().any(|b| pad.pressed(*b)));
    let released = mouse.just_released(MouseButton::Left)
        || touches.any_just_released()
        || skip_keys.iter().any(|k| keys.just_released(*k))
        || pads
            .iter()
            .any(|pad| buttons.iter().any(|b| pad.just_released(*b)));
    let skip = screen.armed && released && !held;
    if !held {
        screen.armed = true;
    }

    let mut finished = false;
    if let Some(duration) = screen.duration {
        screen.elapsed += real.delta_secs();
        let mut playhead = screen.elapsed;
        if let Some(mut sink) = screen.speaker.and_then(|e| sinks.get_mut(e).ok()) {
            if settings.is_changed() {
                sink.set_volume(narration_volume(&settings));
            }
            // The device may start a few buffers later than our real-time clock.
            // Retain the clock as a fallback for absent or stalled devices.
            if !sink.empty() && screen.elapsed < duration + 0.75 {
                playhead = sink.position().as_secs_f32();
            }
        }
        screen.show_frame(playhead / duration, &mut images);
        finished = playhead >= duration;
    } else if !skip {
        screen.prepare(&mut commands, &server, &mut images, &sounds, &settings);
    }
    if skip || finished {
        let root = screen.root;
        if let Some(ghost) = films.pending.pop_front() {
            commands.entity(root).despawn();

            films.current = Some(Screen::spawn(
                &mut commands,
                &ghost,
                &server,
                &mut images,
                &lang,
                &font,
            ));
        } else {
            if pointer_held {
                // Playback and simulation resume immediately. A held pointer
                // still releases onto its original modal surface, not the shop.
                commands
                    .entity(root)
                    .despawn_related::<Children>()
                    .insert(BackgroundColor(Color::NONE));
                if let Some(old) = films.release_guard.replace(root) {
                    commands.entity(old).despawn();
                }
            } else {
                commands.entity(root).despawn();
            }
            films.current = None;
            if !films.previously_paused {
                clock.unpause();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    #[test]
    fn queued_endings_preserve_pause_and_consume_the_release_that_skips() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .init_asset::<AudioSource>()
            .insert_resource(GameData::load())
            .insert_resource(Lang::load("en"))
            .init_resource::<UiFont>()
            .init_resource::<Settings>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .add_plugins(StoryFilmPlugin);

        for paused in [false, true] {
            if paused {
                app.world_mut().resource_mut::<Time<Virtual>>().pause();
            }
            app.world_mut().write_message(PlayStoryFilm("edwin".into()));
            app.world_mut().write_message(PlayStoryFilm("ida".into()));
            app.update();
            let first = app
                .world()
                .resource::<FilmQueue>()
                .current
                .as_ref()
                .unwrap()
                .root;
            assert!(app.world().resource::<Time<Virtual>>().is_paused());
            assert_eq!(app.world().resource::<FilmQueue>().pending.len(), 1);
            for _ in 0..2 {
                app.update();
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left);
                app.update();
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .release(MouseButton::Left);
                app.update();
                app.world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .clear();
                assert!(app.world().get_entity(first).is_err());
            }
            assert!(app.world().resource::<FilmQueue>().current.is_none());
            assert_eq!(app.world().resource::<Time<Virtual>>().is_paused(), paused);
        }

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.world_mut().write_message(PlayStoryFilm("edwin".into()));
        app.update();
        app.world_mut()
            .resource_mut::<FilmQueue>()
            .current
            .as_mut()
            .unwrap()
            .duration = Some(0.000001);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        let guard = app.world().resource::<FilmQueue>().release_guard.unwrap();
        assert!(app.world().resource::<FilmQueue>().current.is_none());
        assert!(app.world().get_entity(guard).is_ok());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(app.world().get_entity(guard).is_err());
    }
}
