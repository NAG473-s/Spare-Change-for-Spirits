//! The score and the three lines spoken over the first crossing.
use super::Crossing;
use crate::{data::asset_exists, i18n::Lang, save::Settings, ui::UiFont};
use bevy::{audio::Volume, prelude::*, text::LineBreak};
use std::time::Duration;

const SCORE: &str = "sounds/intro_music.ogg";
const MUSIC_GAIN: f32 = 0.54;
// Keep each subtitle up through the last syllable of the longest reading.
const LINES: [(f32, f32, &str); 3] = [
    (0.94, 4.86, "intro.story.1"),
    (5.04, 8.66, "intro.story.2"),
    (8.84, 13.15, "intro.story.3"),
];

pub(super) struct Soundtrack {
    score: Option<Handle<AudioSource>>,
    voice: Option<Handle<AudioSource>>,
    playing: Vec<Entity>,
}

impl Soundtrack {
    pub(super) fn load(server: &AssetServer, language: &str, settings: &Settings) -> Self {
        let track = |path: &str, volume| {
            (settings.sound && volume > 0. && asset_exists(path))
                .then(|| server.load(path.to_owned()))
        };
        Self {
            score: track(SCORE, settings.music),
            voice: track(
                &format!("sounds/intro_voice_{language}.ogg"),
                settings.narration,
            ),
            playing: Vec::new(),
        }
    }

    pub(super) fn ready(&self, server: &AssetServer) -> bool {
        [&self.score, &self.voice]
            .into_iter()
            .flatten()
            .all(|clip| {
                server
                    .get_load_state(clip.id())
                    .is_some_and(|state| state.is_loaded() || state.is_failed())
            })
    }

    pub(super) fn play_from(
        &mut self,
        commands: &mut Commands,
        root: Entity,
        server: &AssetServer,
        settings: &Settings,
        elapsed: f32,
    ) {
        for entity in self.playing.drain(..) {
            commands.entity(entity).try_despawn();
        }
        for (clip, gain) in [
            (self.score.as_ref(), settings.music * MUSIC_GAIN),
            (self.voice.as_ref(), settings.narration),
        ] {
            if let Some(clip) = clip.filter(|c| server.is_loaded(c.id())) {
                let player = commands
                    .spawn((
                        AudioPlayer::new(clip.clone()),
                        PlaybackSettings {
                            volume: Volume::Linear(gain),
                            start_position: Some(Duration::from_secs_f32(elapsed)),
                            ..PlaybackSettings::ONCE
                        },
                        ChildOf(root),
                    ))
                    .id();
                self.playing.push(player);
            }
        }
    }
}

#[derive(Component, Default)]
pub(super) struct Subtitle(Option<usize>);

pub(super) fn spawn(commands: &mut Commands, root: Entity, font: &UiFont) {
    commands.spawn((
        Subtitle::default(),
        Text::default(),
        TextFont {
            font: font.regular.clone(),
            font_size: px(20).into(),
            ..default()
        },
        TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
        TextColor(Color::srgb(1., 0.95, 0.83)),
        Node {
            position_type: PositionType::Absolute,
            bottom: percent(3.8),
            max_width: percent(82),
            padding: UiRect::axes(px(13), px(6)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.075, 0.060, 0.095, 0.9)),
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(root),
    ));
}

pub(super) fn update(
    clock: Query<&Crossing>,
    lang: Res<Lang>,
    mut captions: Query<(&mut Subtitle, &mut Text, &mut Visibility)>,
) {
    let Ok(clock) = clock.single() else { return };
    let Ok((mut subtitle, mut text, mut visibility)) = captions.single_mut() else {
        return;
    };
    let line = LINES
        .iter()
        .position(|&(start, end, _)| (start..end).contains(&clock.elapsed));
    if line == subtitle.0 {
        return;
    }
    subtitle.0 = line;
    if let Some(i) = line {
        text.0 = lang.t(LINES[i].2);
        *visibility = Visibility::Inherited;
    } else {
        *visibility = Visibility::Hidden;
    }
}
