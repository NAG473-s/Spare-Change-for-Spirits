//! Postcards from regulars. A regular has `regulars.last_visit` visits in it
//! (`config.ron`); served well on that visit, its last wish is fulfilled and
//! it does not come back. The next dawn its postcard arrives: a picture on
//! the front and a few words on the back (`postcard.text.<n>`).
//!
//! The postcards are kept in the book's Letters chapter, where each can be
//! turned over and one can be put up by the till. The card by the till turns
//! over when it is clicked, and hovering it reads the back.
//!
//! Pictures: `sprites/postcards/<ghost>.png` (the regular's kind), else its
//! portrait on a cream card. The little stand is
//! `models/shop/postcard_stand.glb`, or a wooden block; the card itself is
//! drawn here so it can turn.

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{Phase, Progress};
use crate::i18n::Lang;
use crate::save::Settings;
use crate::shop::ShopLayout;
use crate::tooltip::{HoverTip, TipKind};
use crate::ui::{Action, GameLog, PanelBuilder, UiFont};

pub struct PostcardsPlugin;

impl Plugin for PostcardsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(Phase::Morning),
            deliver.before(crate::save::autosave_dawn),
        )
        .add_systems(
            Update,
            (act, show_card, turn_card, card_tooltip)
                .chain()
                .run_if(not(in_state(Phase::Menu))),
        );
        add_book_refresh(app);
    }
}

/// Redraws the book's postcards after the menus are built: a book page they
/// replace this frame is gone before its cards would be redrawn (otherwise
/// redrawing a despawned card panics).
fn add_book_refresh(app: &mut App) {
    app.add_systems(
        Update,
        refresh_book_cards
            .after(act)
            .after(crate::menu::MenuSystems),
    );
}

/// A postcard in the journal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Postcard {
    /// The regular who sent it (`regular.name.<n>`) and its kind.
    pub name: u32,
    pub ghost: String,
    /// The night of its last wish, and what it was.
    pub night: u32,
    pub item: String,
    /// Showing its back (the words) rather than the picture.
    pub turned: bool,
}

impl Postcard {
    /// The words on the back.
    pub fn text(&self, lang: &Lang) -> String {
        let variants = lang.count("postcard.text.").max(1) as u32;
        let n = self.name % variants + 1;
        lang.f(
            &format!("postcard.text.{n}"),
            &[
                ("who", &lang.t(&format!("regular.name.{}", self.name))),
                ("item", &lang.product(&self.item)),
                ("night", &self.night),
            ],
        )
    }

    pub fn from(&self, lang: &Lang) -> String {
        lang.f(
            "postcard.from",
            &[("who", &lang.t(&format!("regular.name.{}", self.name)))],
        )
    }

    /// The picture on the front.
    fn picture(&self) -> Option<String> {
        [
            format!("sprites/postcards/{}.png", self.ghost),
            format!("sprites/ghosts/{}.png", self.ghost),
        ]
        .into_iter()
        .find(|p| asset_exists(p))
    }
}

/// A regular served well on its last visit: its last wish is fulfilled.
/// Returns true when this was it (the caller says so in the log).
pub fn last_wish(data: &GameData, progress: &mut Progress, regular: usize) -> bool {
    let night = progress.night;
    let Some(r) = progress.journal.regulars.get_mut(regular) else {
        return false;
    };
    if r.farewell.is_some() || r.visits < data.config.regulars.last_visit {
        return false;
    }
    r.farewell = Some(night);
    true
}

/// At dawn: the postcards of tonight's last wishes arrive.
fn deliver(lang: Res<Lang>, mut progress: ResMut<Progress>, mut log: ResMut<GameLog>) {
    let night = progress.night;
    let arriving: Vec<Postcard> = progress
        .journal
        .regulars
        .iter()
        .filter(|r| r.farewell == Some(night))
        .filter(|r| {
            !progress
                .journal
                .postcards
                .iter()
                .any(|p| p.name == r.name && p.ghost == r.ghost)
        })
        .map(|r| Postcard {
            name: r.name,
            ghost: r.ghost.clone(),
            night,
            item: r
                .last_purchase
                .clone()
                .unwrap_or_else(|| r.favourite.clone()),
            turned: false,
        })
        .collect();
    for card in arriving {
        log.push(lang.f(
            "log.postcard",
            &[("who", &lang.t(&format!("regular.name.{}", card.name)))],
        ));
        progress.journal.postcards.push(card);
        // The first one goes up by the till by itself.
        if progress.journal.displayed_postcard.is_none() {
            progress.journal.displayed_postcard = Some(progress.journal.postcards.len() - 1);
        }
    }
}

fn act(mut actions: MessageReader<Action>, mut progress: ResMut<Progress>) {
    for action in actions.read() {
        match action {
            Action::TurnPostcard(i) => {
                if let Some(card) = progress.journal.postcards.get_mut(*i) {
                    card.turned = !card.turned;
                }
            }
            Action::DisplayPostcard(i) if *i < progress.journal.postcards.len() => {
                progress.journal.displayed_postcard = Some(*i);
            }
            _ => {}
        }
    }
}

#[derive(Component)]
struct BookPostcard {
    index: usize,
    /// The back and the by-the-till check mark last drawn in this entry.
    rendered: Option<(bool, bool)>,
}

/// Keep one entry per postcard so turning it leaves the scrolling page in place.
pub fn book_section(ui: &mut PanelBuilder, lang: &Lang, journal: &crate::book::Journal) {
    if journal.postcards.is_empty() {
        return;
    }
    ui.rule();
    ui.section(&lang.t("postcard.title"));
    for index in 0..journal.postcards.len() {
        ui.node(
            ui.root(),
            (
                BookPostcard {
                    index,
                    rendered: None,
                },
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: px(10),
                    ..default()
                },
            ),
        );
    }
}

fn refresh_book_cards(
    mut commands: Commands,
    mut entries: Query<(Entity, &mut BookPostcard)>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    assets: Res<AssetServer>,
) {
    for (root, mut entry) in &mut entries {
        let i = entry.index;
        let Some(card) = progress.journal.postcards.get(i) else {
            continue;
        };
        let shown = progress.journal.displayed_postcard == Some(i);
        let state = Some((card.turned, shown));
        if entry.rendered == state {
            continue;
        }
        entry.rendered = state;
        let mut ui = PanelBuilder::new(&mut commands, root, &font);
        ui.text(&card.from(&lang));
        if card.turned {
            ui.text(&card.text(&lang));
        } else if let Some(path) = card.picture() {
            ui.image_sized(root, assets.load(path), BOOK_PICTURE);
        }
        let row = ui.row();
        ui.button_in(
            row,
            &lang.t(if card.turned {
                "postcard.turn_front"
            } else {
                "postcard.turn_back"
            }),
            Action::TurnPostcard(i),
            false,
            true,
        );
        ui.button_in(
            row,
            &lang.t(if shown {
                "postcard.displayed"
            } else {
                "postcard.display"
            }),
            Action::DisplayPostcard(i),
            shown,
            true,
        );
    }
}

/// Size of a postcard's picture in the book (wide, 3 : 2).
const BOOK_PICTURE: Vec2 = Vec2::new(210.0, 140.0);
/// The card by the till: where it stands on the counter, its size and the
/// height of its stand. It stands in the middle of the counter, turned
/// a little towards the customers and leaning back towards the view.
const CARD_AT: Vec3 = Vec3::new(6.22, 0.742, -0.38);
const CARD_TURN: f32 = -0.45;
const CARD_LEAN: f32 = -0.25;
const CARD_SIZE: Vec2 = Vec2::new(0.36, 0.24);
const STAND_SIZE: Vec3 = Vec3::new(0.2, 0.04, 0.07);
const STAND_MODEL: &str = "models/shop/postcard_stand.glb";
const CARD_PAPER: Color = Color::srgb(0.93, 0.88, 0.76);
/// How tall the card stands above the counter top, and how long a turn takes.
const CARD_LIFT: f32 = 0.15;
const TURN_SECS: f32 = 0.45;
/// Lift the lower corners clear of the wooden lips while the card turns.
const TURN_LIFT: f32 = 0.07;

/// The card by the till (its index in the journal).
#[derive(Component)]
struct TillCard(usize);

/// The turning part of the card, and the angle it is at.
#[derive(Component)]
struct CardFace {
    angle: f32,
}

#[derive(Component)]
struct HoveredCard;

/// Builds the card for the postcard on display (and replaces it when another
/// one is chosen).
#[allow(clippy::too_many_arguments)]
fn show_card(
    mut commands: Commands,
    progress: Res<Progress>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shown: Query<(Entity, &TillCard)>,
    layout: Option<Res<ShopLayout>>,
) {
    let wanted = progress
        .journal
        .displayed_postcard
        .filter(|i| *i < progress.journal.postcards.len());
    let now = shown.iter().next().map(|(_, c)| c.0);
    if wanted == now || layout.is_none() {
        return;
    }
    for (e, _) in &shown {
        commands.entity(e).despawn();
    }
    let Some(i) = wanted else {
        return;
    };
    let card = &progress.journal.postcards[i];
    let root = commands
        .spawn((
            TillCard(i),
            Transform::from_translation(CARD_AT).with_rotation(Quat::from_rotation_y(CARD_TURN)),
            Visibility::default(),
        ))
        .id();
    if asset_exists(STAND_MODEL) {
        commands.spawn((
            WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(STAND_MODEL))),
            Transform::default(),
            ChildOf(root),
        ));
    } else {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(STAND_SIZE))),
            MeshMaterial3d(materials.add(Color::srgb(0.36, 0.24, 0.15))),
            Transform::from_xyz(0.0, STAND_SIZE.y / 2.0, 0.0),
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    // Keep the bottom edge of the tilted card centred in the stand's groove.
    let middle_z = 0.5 * CARD_SIZE.y * CARD_LEAN.sin();
    let lean = commands
        .spawn((
            Transform::from_xyz(0.0, CARD_LIFT, middle_z)
                .with_rotation(Quat::from_rotation_x(CARD_LEAN)),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    let face = commands
        .spawn((
            CardFace {
                angle: if card.turned {
                    std::f32::consts::PI
                } else {
                    0.0
                },
            },
            Transform::default(),
            Visibility::default(),
            ChildOf(lean),
        ))
        .observe(on_card_click)
        .observe(|over: On<PointerOver>, mut commands: Commands| {
            commands.entity(over.event_target()).insert(HoveredCard);
        })
        .observe(
            |out: On<PointerOut>, mut commands: Commands, mut tip: ResMut<HoverTip>| {
                commands.entity(out.event_target()).remove::<HoveredCard>();
                HoverTip::hide(&mut tip);
            },
        )
        .id();
    let quad = meshes.add(Rectangle::new(CARD_SIZE.x, CARD_SIZE.y));
    let front = materials.add(StandardMaterial {
        base_color: if card.picture().is_some() {
            Color::WHITE
        } else {
            CARD_PAPER
        },
        base_color_texture: card.picture().map(|p| asset_server.load(p)),
        alpha_mode: AlphaMode::Mask(0.5),
        unlit: true,
        ..default()
    });
    let paper = materials.add(StandardMaterial {
        base_color: CARD_PAPER,
        unlit: true,
        ..default()
    });
    // Paper behind the picture (a portrait has transparent edges), the
    // picture on it, and the back with a stamp.
    for (material, z, turn) in [
        (paper.clone(), 0.001, 0.0),
        (front, 0.002, 0.0),
        (paper, -0.001, std::f32::consts::PI),
    ] {
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.0, z).with_rotation(Quat::from_rotation_y(turn)),
            NotShadowCaster,
            ChildOf(face),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(0.035, 0.045))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.25, 0.22),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(-0.12, 0.07, -0.002)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        NotShadowCaster,
        Pickable::IGNORE,
        ChildOf(face),
    ));
    // A larger, invisible box makes the small card easy to click.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(CARD_SIZE.x * 1.4, CARD_SIZE.y * 1.6, 0.08))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::NONE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        NotShadowCaster,
        ChildOf(face),
    ));
}

fn on_card_click(
    click: On<PointerClick>,
    cards: Query<&TillCard>,
    parents: Query<&ChildOf>,
    mut actions: MessageWriter<Action>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let card = parents
        .iter_ancestors(click.event_target())
        .find_map(|e| cards.get(e).ok());
    if let Some(card) = card {
        actions.write(Action::TurnPostcard(card.0));
    }
}

/// The card turns over to the side the journal says (at once with
/// *Reduced motion*).
fn turn_card(
    time: Res<Time>,
    progress: Res<Progress>,
    settings: Res<Settings>,
    cards: Query<&TillCard>,
    parents: Query<&ChildOf>,
    mut faces: Query<(Entity, &mut CardFace, &mut Transform)>,
) {
    for (entity, mut face, mut transform) in &mut faces {
        let Some(card) = parents
            .iter_ancestors(entity)
            .find_map(|e| cards.get(e).ok())
        else {
            continue;
        };
        let turned = progress
            .journal
            .postcards
            .get(card.0)
            .is_some_and(|c| c.turned);
        let goal = if turned { std::f32::consts::PI } else { 0.0 };
        if settings.reduced_motion {
            face.angle = goal;
        } else {
            let step = std::f32::consts::PI / TURN_SECS * time.delta_secs();
            face.angle += (goal - face.angle).clamp(-step, step);
        }
        transform.rotation = Quat::from_rotation_y(face.angle);
        transform.translation.y = TURN_LIFT * face.angle.sin().max(0.0).sqrt();
    }
}

/// Hovering the card: who sent it, the words on its back, and what a click does.
fn card_tooltip(
    hovered: Query<Entity, With<HoveredCard>>,
    parents: Query<&ChildOf>,
    cards: Query<&TillCard>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    mut tip: ResMut<HoverTip>,
) {
    let Some(card) = hovered
        .iter()
        .next()
        .and_then(|e| parents.iter_ancestors(e).find_map(|a| cards.get(a).ok()))
        .and_then(|c| progress.journal.postcards.get(c.0))
    else {
        return;
    };
    let mut t = HoverTip {
        title: card.from(&lang),
        ..default()
    };
    t.line(TipKind::Text, card.text(&lang));
    t.line(TipKind::Hint, lang.t("postcard.click_hint"));
    HoverTip::show(&mut tip, t);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn regular(visits: u32) -> crate::book::Regular {
        crate::book::Regular {
            ghost: "shy".into(),
            name: 1,
            visits,
            favourite: "lavender".into(),
            last_purchase: Some("tea_tin".into()),
            seen_keepsakes: Vec::new(),
            encounters: Vec::new(),
            farewell: None,
        }
    }

    #[test]
    fn a_regulars_last_wish_brings_one_postcard_at_dawn() {
        let data = GameData::load();
        let last = data.config.regulars.last_visit;
        let mut progress = Progress::new(0);
        progress.night = 12;
        progress.journal.regulars.push(regular(last - 1));
        assert!(!last_wish(&data, &mut progress, 0), "not yet");
        progress.journal.regulars[0].visits = last;
        assert!(last_wish(&data, &mut progress, 0));
        assert!(!last_wish(&data, &mut progress, 0), "only once");
        let mut world = World::new();
        world.insert_resource(progress);
        world.insert_resource(Lang::load("en"));
        world.init_resource::<GameLog>();
        world.run_system_once(deliver).unwrap();
        world.run_system_once(deliver).unwrap();
        let journal = &world.resource::<Progress>().journal;
        assert_eq!(journal.postcards.len(), 1);
        let card = &journal.postcards[0];
        assert_eq!(
            (card.name, card.night, card.item.as_str()),
            (1, 12, "tea_tin")
        );
        assert_eq!(
            journal.displayed_postcard,
            Some(0),
            "it goes up by the till"
        );
        let text = card.text(&Lang::load("en"));
        assert!(
            text.contains("Mrs Wispel") || text.contains("Tea"),
            "{text}"
        );
        // It does not come back as a customer, nor leave notes.
        let mut progress = world.resource::<Progress>().clone();
        let mut rng = crate::rng::Rng::new(1);
        for _ in 0..50 {
            assert_eq!(
                crate::regulars::returning(
                    &data,
                    &mut progress,
                    "shy",
                    std::iter::empty(),
                    &mut rng
                ),
                None
            );
        }
    }

    #[test]
    fn every_postcard_text_names_the_regular() {
        let lang = Lang::load("en");
        let n = lang.count("postcard.text.");
        assert!(n >= 3);
        for i in 1..=n {
            assert!(lang.t(&format!("postcard.text.{i}")).contains("{item}"));
        }
    }

    #[test]
    fn flipping_and_displaying_postcards_updates_the_open_book_in_place() {
        let lang = Lang::load("en");
        let mut progress = Progress::new(0);
        for (name, ghost) in [(1, "shy"), (2, "cat")] {
            progress.journal.postcards.push(Postcard {
                name,
                ghost: ghost.into(),
                item: "tea_tin".into(),
                ..default()
            });
        }
        let message = progress.journal.postcards[0].text(&lang);
        progress.journal.displayed_postcard = Some(0);
        let mut app = App::new();
        let page = app
            .world_mut()
            .spawn(ScrollPosition(Vec2::new(0., 120.)))
            .id();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .insert_resource(lang)
            .init_resource::<UiFont>()
            .insert_resource(progress)
            .add_message::<Action>()
            .add_systems(
                Startup,
                move |mut commands: Commands,
                      font: Res<UiFont>,
                      lang: Res<Lang>,
                      progress: Res<Progress>| {
                    book_section(
                        &mut PanelBuilder::new(&mut commands, page, &font),
                        &lang,
                        &progress.journal,
                    );
                },
            )
            .add_systems(Update, (act, refresh_book_cards).chain());
        let marked_card = |world: &mut World| {
            world
                .query::<(&crate::ui::ActionButton, &Children)>()
                .iter(world)
                .find_map(|(button, children)| match button.0 {
                    Action::DisplayPostcard(i)
                        if world.get::<Text>(children[0])?.0.starts_with('✓') =>
                    {
                        Some(i)
                    }
                    _ => None,
                })
        };
        app.update();
        assert_eq!(marked_card(app.world_mut()), Some(0));
        app.world_mut().write_message(Action::TurnPostcard(0));
        app.world_mut().write_message(Action::DisplayPostcard(1));
        app.update();
        let world = app.world_mut();
        assert_eq!(marked_card(world), Some(1));
        assert!(world.query::<&Text>().iter(world).any(|t| t.0 == message));
        assert_eq!(world.query::<&ImageNode>().iter(world).count(), 1);
        assert_eq!(world.get::<ScrollPosition>(page).unwrap().y, 120.);
    }

    /// Opening the Letters page rebuilds the book while the cards of the
    /// page before are still waiting to be drawn; they must not be redrawn
    /// after the page has replaced them (the game used to panic here).
    #[test]
    fn rebuilding_the_book_page_while_cards_wait_does_not_panic() {
        let mut progress = Progress::new(0);
        progress.journal.postcards.push(Postcard {
            name: 1,
            ghost: "shy".into(),
            item: "tea_tin".into(),
            ..default()
        });
        let mut app = App::new();
        let page = app.world_mut().spawn(Node::default()).id();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .insert_resource(Lang::load("en"))
            .init_resource::<UiFont>()
            .insert_resource(progress)
            .add_message::<Action>()
            // The menu builds the Letters page on every frame, as it does
            // when the page is opened and rebuilt right after.
            .add_systems(
                Update,
                (move |mut commands: Commands,
                       font: Res<UiFont>,
                       lang: Res<Lang>,
                       progress: Res<Progress>| {
                    book_section(
                        &mut PanelBuilder::new(&mut commands, page, &font),
                        &lang,
                        &progress.journal,
                    );
                })
                .in_set(crate::menu::MenuSystems),
            )
            .add_systems(Update, act);
        add_book_refresh(&mut app);
        for _ in 0..5 {
            app.update();
        }
        let world = app.world_mut();
        let cards = world.query::<&BookPostcard>().iter(world).count();
        assert_eq!(cards, 1, "one card on the page");
        let drawn = world
            .query::<&BookPostcard>()
            .iter(world)
            .all(|card| card.rendered.is_some());
        assert!(drawn, "the card on the current page is drawn");
    }
}
