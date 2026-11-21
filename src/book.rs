//! The ghost book: a page for every kind of ghost the shop has met, filled in
//! while playing – how often it came and was served, what it likes, how it
//! pays and which wishes it has whispered so far – with the regulars, Hedda's
//! letters and the keepsakes. Opened from the pause menu or the evening panel.
//!
//! The book is one overlay with coloured chapter tabs along the top (ghosts,
//! keepsakes, letters, order slips, the first-sale guide and, in the full
//! game, the achievements), so new pages join the book instead of the menus.
//!
//! The book lives in the save as part of [`Journal`], together with the
//! regulars (`regulars.rs`) and the ware in the shop window (`showcase.rs`).

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{GameData, GhostDef, asset_exists};
use crate::i18n::Lang;
use crate::shop::{ShopLayout, Storeroom};
use crate::ui::{Action, PANEL_EDGE, PanelBuilder, TEXT};
use crate::upgrades::Upgrades;

/// What the shop remembers about its customers (saved with the game).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Journal {
    pub order_history: Vec<crate::orders::Order>,
    pub first_sale: crate::guide::FirstSale,
    pub first_sale_checkpoint: Option<crate::guide::FirstSaleCheckpoint>,
    /// One entry per kind of ghost met (ghost id).
    pub book: BTreeMap<String, BookEntry>,
    /// Ghosts that come back by name.
    pub regulars: Vec<Regular>,
    /// The ware shown in the shop window (product id).
    pub showcase: Option<String>,
    /// The night each chapter of a story was finished (story index → nights
    /// in chapter order), for logbook keepsakes.
    pub chapter_nights: BTreeMap<usize, Vec<u32>>,
    /// Finale guests whose farewell letter has been read (`finale.rs`).
    pub finale_letters: Vec<String>,
    /// Stories whose parcel tonight is a ware set aside from the player's
    /// own stock (the others come from the ferryman).
    pub set_aside: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BookEntry {
    /// Times it came into the shop.
    pub met: u32,
    /// Times it was sold something.
    pub served: u32,
    /// The wishes it has whispered so far (product ids, in order).
    pub wishes: Vec<String>,
    /// Distinct products actually sold to this kind, including sales by the apprentice.
    pub fulfilled_wishes: Vec<String>,
}

/// A regular customer: one ghost of its kind with a name of its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Regular {
    pub ghost: String,
    /// Its name: `regular.name.<n>`.
    pub name: u32,
    pub visits: u32,
    /// The ware it bought the first time; it often asks for it again.
    pub favourite: String,
    #[serde(default)]
    pub last_purchase: Option<String>,
    /// Only keepsakes this customer has actually seen on the shop floor.
    #[serde(default)]
    pub seen_keepsakes: Vec<String>,
    #[serde(default)]
    pub encounters: Vec<crate::regulars::Encounter>,
}

impl Regular {
    pub fn name(&self, lang: &Lang) -> String {
        lang.t(&format!("regular.name.{}", self.name))
    }
}

impl Journal {
    pub fn has_seen(&self, ghost: &str) -> bool {
        self.book.contains_key(ghost)
    }

    fn note(products: &mut Vec<String>, product: &str) -> bool {
        if product.is_empty() || products.iter().any(|p| p == product) {
            return false;
        }
        products.push(product.to_owned());
        true
    }

    /// Nightly sales and visits replay from the checkpoint, knowledge does not.
    pub fn remember_discoveries(&mut self, live: &Journal) -> bool {
        let mut changed = false;
        for (kind, observed) in &live.book {
            let saved = if let Some(saved) = self.book.get_mut(kind) {
                saved
            } else {
                changed = true;
                self.book.entry(kind.clone()).or_default()
            };
            for product in &observed.wishes {
                changed |= Self::note(&mut saved.wishes, product);
            }
            for product in &observed.fulfilled_wishes {
                changed |= Self::note(&mut saved.fulfilled_wishes, product);
            }
        }
        changed
    }

    /// Recover the products proven by older saves that counted sales without storing their wares.
    pub fn recover_recorded_sales(&mut self) {
        for entry in self.book.values_mut() {
            if entry.served > 0
                && let [only] = entry.wishes.as_slice()
            {
                Self::note(&mut entry.fulfilled_wishes, only);
            }
        }
        for regular in &self.regulars {
            let entry = self.book.entry(regular.ghost.clone()).or_default();
            Self::note(&mut entry.fulfilled_wishes, &regular.favourite);
            for product in regular
                .encounters
                .iter()
                .filter_map(|e| e.recalled_purchase())
            {
                Self::note(&mut entry.fulfilled_wishes, product);
            }
            if let Some(product) = &regular.last_purchase {
                Self::note(&mut entry.fulfilled_wishes, product);
            }
        }
        for order in &self.order_history {
            if order.state == crate::orders::OrderState::Collected {
                let entry = self.book.entry(order.ghost.clone()).or_default();
                Self::note(&mut entry.fulfilled_wishes, &order.product);
            }
        }
    }

    /// A ghost was seen. Customers may already have made their wish known, e.g. by placing an order.
    pub fn met(&mut self, ghost: &str, wish: &str) {
        let entry = self.book.entry(ghost.to_string()).or_default();
        entry.met += 1;
        Self::note(&mut entry.wishes, wish);
    }

    pub fn heard(&mut self, ghost: &str, wish: &str) {
        Self::note(
            &mut self.book.entry(ghost.to_owned()).or_default().wishes,
            wish,
        );
    }

    pub fn note_fulfilled(&mut self, ghost: &str, product: &str) {
        Self::note(
            &mut self
                .book
                .entry(ghost.to_owned())
                .or_default()
                .fulfilled_wishes,
            product,
        );
    }

    /// A ghost was sold something.
    pub fn served(&mut self, ghost: &str, bought: &str, regular: Option<usize>) {
        let entry = self.book.entry(ghost.to_string()).or_default();
        entry.served += 1;
        Self::note(&mut entry.fulfilled_wishes, bought);
        if let Some(customer) = regular.and_then(|i| self.regulars.get_mut(i)) {
            customer.last_purchase = Some(bought.to_owned());
        }
    }

    pub fn regular(&self, ghost: &str) -> Option<&Regular> {
        self.regulars.iter().find(|r| r.ghost == ghost)
    }
}

/// The book's chapters: one coloured tab each along the top of the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chapter {
    Ghosts,
    Keepsakes,
    Letters,
    Orders,
    Guide,
    /// Full game only.
    Achievements,
}

impl Chapter {
    fn all() -> impl Iterator<Item = Chapter> {
        [
            Chapter::Ghosts,
            Chapter::Keepsakes,
            Chapter::Letters,
            Chapter::Orders,
            Chapter::Guide,
            Chapter::Achievements,
        ]
        .into_iter()
        // The demo has no achievements.
        .filter(|c| *c != Chapter::Achievements || !cfg!(feature = "demo"))
    }

    fn key(self) -> &'static str {
        match self {
            Chapter::Ghosts => "book.tab.ghosts",
            Chapter::Keepsakes => "book.tab.keepsakes",
            Chapter::Letters => "book.tab.letters",
            Chapter::Orders => "book.tab.orders",
            Chapter::Guide => "book.tab.guide",
            Chapter::Achievements => "book.tab.achievements",
        }
    }

    /// The tab's ribbon colour (the tab also carries its name, so the colour
    /// is never the only sign).
    fn color(self) -> Color {
        match self {
            Chapter::Ghosts => Color::srgb_u8(150, 140, 220),
            Chapter::Keepsakes => Color::srgb_u8(226, 168, 82),
            Chapter::Letters => Color::srgb_u8(214, 120, 140),
            Chapter::Orders => Color::srgb_u8(120, 182, 120),
            Chapter::Guide => Color::srgb_u8(108, 164, 214),
            Chapter::Achievements => Color::srgb_u8(236, 206, 96),
        }
    }
}

/// Height of a tab's coloured ribbon.
const RIBBON: f32 = 6.0;

/// The row of chapter tabs at the top of every book page, and the button
/// that closes the book.
pub fn tabs(ui: &mut PanelBuilder, lang: &Lang, current: Chapter) {
    let root = ui.root();
    let row = ui.node(
        root,
        Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(4),
            row_gap: px(4),
            align_items: AlignItems::End,
            ..default()
        },
    );
    for chapter in Chapter::all() {
        let tab = ui.icon_button_in(
            row,
            None,
            &lang.t(chapter.key()),
            Action::BookChapter(chapter),
            chapter == current,
            true,
        );
        // The open chapter's tab stands a little taller, like a bookmark;
        // the label sits below the ribbon.
        let height = if chapter == current {
            RIBBON * 2.0
        } else {
            RIBBON
        };
        ui.adjust(tab, move |node| node.padding.top = px(height + 4.0));
        ui.node(
            tab,
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(0),
                    left: px(0),
                    right: px(0),
                    height: px(height),
                    border_radius: BorderRadius::top(px(4)),
                    ..default()
                },
                BackgroundColor(chapter.color()),
                Pickable::IGNORE,
            ),
        );
    }
    let close = ui.icon_button_in(
        row,
        None,
        &lang.t("book.close"),
        Action::BookClose,
        false,
        true,
    );
    ui.adjust(close, |node| node.margin.left = auto());
    ui.rule();
}

fn seen_ghosts<'a>(
    data: &'a GameData,
    journal: &'a Journal,
) -> impl Iterator<Item = (usize, &'a GhostDef)> {
    data.ghosts
        .iter()
        .enumerate()
        .filter(|(_, ghost)| journal.has_seen(&ghost.id))
}

/// Hedda's letters found so far: the first, and those behind cleared junk.
pub fn letters_found(data: &GameData, upgrades: &Upgrades) -> Vec<u32> {
    let mut found = vec![1];
    found.extend(
        data.upgrades
            .iter()
            .filter(|u| upgrades.has(&u.id))
            .filter_map(|u| u.letter),
    );
    found.sort();
    found.dedup();
    found
}

const RECENT_ENCOUNTERS: usize = 3;
const HISTORY_PAGE_SIZE: usize = 8;

pub fn regular_history_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    journal: &Journal,
    name: u32,
    page: usize,
) {
    let Some(regular) = journal
        .regulars
        .iter()
        .find(|r| r.name == name && journal.has_seen(&r.ghost))
    else {
        ui.menu_option(ui.root(), &lang.t("menu.back"), Action::BookBack, false);
        return;
    };
    ui.heading(&regular.name(lang));
    ui.menu_option(ui.root(), &lang.t("menu.back"), Action::BookBack, false);
    let skip = RECENT_ENCOUNTERS + page * HISTORY_PAGE_SIZE;
    let mut entries = regular.encounters.iter().rev().skip(skip);
    for encounter in entries.by_ref().take(HISTORY_PAGE_SIZE) {
        ui.text(&encounter.entry(lang));
    }
    if entries.next().is_some() {
        ui.menu_option(
            ui.root(),
            &lang.t("regular.earlier_visits"),
            Action::RegularHistory(name, page + 1),
            false,
        );
    }
}

/// Only guests the player has actually seen have a name and a link in this index.
pub fn book_page(ui: &mut PanelBuilder, lang: &Lang, data: &GameData, journal: &Journal) {
    let met = seen_ghosts(data, journal).count();
    ui.heading(&lang.f(
        "book.title",
        &[("met", &met), ("total", &data.ghosts.len())],
    ));
    let root = ui.root();
    if met == 0 {
        ui.dim(&lang.t("book.empty"));
    } else {
        let index = ui.node(
            root,
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                column_gap: px(6),
                row_gap: px(5),
                ..default()
            },
        );
        for (id, ghost) in seen_ghosts(data, journal) {
            ui.menu_option(
                index,
                &lang.ghost(&ghost.id),
                Action::GhostProfile(id),
                false,
            );
        }
    }
}

/// A sheet can supply the first idle frame if the individual portrait is missing.
fn portrait(ghost: &str, assets: &AssetServer) -> Option<ImageNode> {
    let still = format!("sprites/ghosts/{ghost}.png");
    if asset_exists(&still) {
        return Some(ImageNode::new(assets.load(still)));
    }
    let sheet = format!("sprites/ghosts/{ghost}_sheet.png");
    asset_exists(&sheet).then(|| ImageNode {
        rect: Some(Rect::new(0., 0., 176., 264.)),
        ..ImageNode::new(assets.load(sheet))
    })
}

pub fn ghost_profile(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    journal: &Journal,
    index: usize,
    assets: &AssetServer,
) {
    let Some(ghost) = data.ghosts.get(index).filter(|g| journal.has_seen(&g.id)) else {
        book_page(ui, lang, data, journal);
        return;
    };
    let root = ui.root();
    ui.heading(&lang.ghost(&ghost.id));
    ui.menu_option(root, &lang.t("menu.back"), Action::BookBack, false);
    let header = ui.node(
        root,
        Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(16),
            row_gap: px(8),
            align_items: AlignItems::Center,
            ..default()
        },
    );
    if let Some(image) = portrait(&ghost.id, assets) {
        let mat = ui.node(
            header,
            (
                Node {
                    width: px(150),
                    max_width: percent(100),
                    aspect_ratio: Some(2.0 / 3.0),
                    padding: UiRect::all(px(6)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.12, 0.17)),
                BorderColor::all(PANEL_EDGE),
            ),
        );
        ui.node(
            mat,
            (
                image,
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                Pickable::IGNORE,
            ),
        );
    }
    let notes = ui.node(
        header,
        Node {
            flex_basis: px(140),
            flex_grow: 1.0,
            min_width: px(0),
            ..default()
        },
    );
    ui.add_text(
        notes,
        &lang.t(&format!("book.note.{}", ghost.id)),
        false,
        16.0,
        TEXT,
    );
    let Some(entry) = journal.book.get(&ghost.id) else {
        return;
    };
    if entry.met > 0 || entry.served > 0 {
        ui.text(&lang.f(
            "book.visits",
            &[("met", &entry.met), ("served", &entry.served)],
        ));
    }
    let named_products =
        |ids: &[String]| lang.join(&ids.iter().map(|id| lang.product(id)).collect::<Vec<_>>());
    if !entry.wishes.is_empty() {
        let possible: Vec<&str> = match data.stories.iter().find(|s| s.ghost == ghost.id) {
            Some(story) => story
                .chapters
                .iter()
                .filter(|chapter| !chapter.fare)
                .map(|chapter| chapter.wish.as_str())
                .collect(),
            None => ghost.wishes.iter().map(String::as_str).collect(),
        };
        let total = possible
            .into_iter()
            .chain(entry.wishes.iter().map(String::as_str))
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        ui.text(&lang.f(
            "book.wishes",
            &[
                ("items", &named_products(&entry.wishes)),
                ("seen", &entry.wishes.len()),
                ("total", &total),
            ],
        ));
    }
    if !entry.fulfilled_wishes.is_empty() {
        ui.text(&lang.f(
            "book.fulfilled",
            &[("items", &named_products(&entry.fulfilled_wishes))],
        ));
    }
    let mut about: Vec<String> = Vec::new();
    if ghost.id == crate::ghosts::THIEF_GHOST {
        about.push(lang.t("ghost_tip.thief"));
    } else if !data
        .stories
        .iter()
        .any(|s| s.finale_guest.as_deref() == Some(&ghost.id))
    {
        about.extend(ghost.needs.iter().map(|need| need.describe(lang)));
        if !ghost.pays_with.is_empty() {
            let coins: Vec<_> = ghost.pays_with.iter().map(|c| c.name(lang)).collect();
            about.push(lang.f("ghost_tip.pays_with", &[("coins", &lang.join(&coins))]));
        }
        let premium = ((ghost.pay_multiplier - 1.0) * 100.0).round() as i32;
        if premium != 0 {
            let key = if premium > 0 {
                "ghost_tip.pays_more"
            } else {
                "ghost_tip.pays_less"
            };
            about.push(lang.f(key, &[("percent", &premium.abs())]));
        }
        for (chance, key) in [
            (ghost.quirks.knock_chance, "ghost_tip.knocks"),
            (ghost.quirks.counterfeit_chance, "ghost_tip.counterfeit"),
            (ghost.quirks.double_pay_chance, "ghost_tip.double_pay"),
        ] {
            if chance > 0.0 {
                about.push(lang.t(key));
            }
        }
    }
    if !about.is_empty() {
        ui.section(&lang.t("book.observations"));
        for line in about {
            ui.dim(&line);
        }
    }
    if let Some(r) = journal.regular(&ghost.id) {
        ui.section(&lang.f(
            "book.regular",
            &[
                ("name", &r.name(lang)),
                ("visits", &r.visits),
                ("item", &lang.product(&r.favourite)),
            ],
        ));
        for encounter in r.encounters.iter().rev().take(RECENT_ENCOUNTERS) {
            ui.text(&encounter.entry(lang));
        }
        if r.encounters.len() > RECENT_ENCOUNTERS {
            ui.menu_option(
                root,
                &lang.t("regular.earlier_visits"),
                Action::RegularHistory(r.name, 0),
                false,
            );
        }
    }
}

pub fn letters_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    journal: &Journal,
    upgrades: &Upgrades,
) {
    ui.heading(&lang.t("book.letters"));
    for n in letters_found(data, upgrades) {
        ui.rule();
        ui.text(&lang.t(&format!("letter.{n}")));
    }
    // The farewell letters left at a finale, to read again.
    for guest in &journal.finale_letters {
        ui.rule();
        ui.text(&lang.t(&format!("finale.{guest}.letter")));
    }
}

/// Where a keepsake came from: (story index, chapter number from 1).
fn keepsake_origin(data: &GameData, id: &str) -> Option<(usize, usize)> {
    data.stories.iter().enumerate().find_map(|(i, story)| {
        story
            .chapters
            .iter()
            .position(|c| c.keepsake.as_deref() == Some(id))
            .map(|c| (i, c + 1))
    })
}

/// Keepsakes the player owns: standing in the shop or in the storeroom.
fn owned_keepsakes<'a>(
    data: &'a GameData,
    layout: &'a ShopLayout,
    storeroom: &'a Storeroom,
) -> impl Iterator<Item = (usize, &'a crate::data::DecorDef)> {
    data.decor
        .iter()
        .enumerate()
        .filter(move |(_, d)| d.keepsake && (storeroom.count(&d.id) > 0 || placed(layout, &d.id)))
}

fn placed(layout: &ShopLayout, id: &str) -> bool {
    layout.cells.iter().any(|c| c.decor.as_deref() == Some(id))
}

/// Size of a keepsake's picture on its page.
const KEEPSAKE_PICTURE: f32 = 96.0;

/// The keepsakes chapter: each one the player owns opens its page.
pub fn keepsakes_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    (layout, storeroom): (&ShopLayout, &Storeroom),
    assets: &AssetServer,
) {
    ui.heading(&lang.t("book.keepsakes"));
    let owned: Vec<_> = owned_keepsakes(data, layout, storeroom).collect();
    if owned.is_empty() {
        ui.dim(&lang.t("book.no_keepsakes"));
        return;
    }
    ui.dim(&lang.t("book.keepsakes_hint"));
    let root = ui.root();
    let grid = ui.node(
        root,
        Node {
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
            column_gap: px(6),
            row_gap: px(5),
            ..default()
        },
    );
    for (i, d) in owned {
        let icon = format!("sprites/decor/{}.png", d.id);
        let icon = asset_exists(&icon).then(|| assets.load(icon));
        ui.icon_button_in(
            grid,
            icon,
            &lang.decor(&d.id),
            Action::KeepsakePage(i),
            false,
            true,
        );
    }
}

/// One keepsake's page: its picture, who left it and the words they said
/// when they gave it. Only chapters already lived through are told.
pub fn keepsake_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &crate::day_cycle::Progress,
    (layout, storeroom): (&ShopLayout, &Storeroom),
    index: usize,
    assets: &AssetServer,
) {
    let journal = &progress.journal;
    let Some(d) = data
        .decor
        .get(index)
        .filter(|d| owned_keepsakes(data, layout, storeroom).any(|(_, o)| o.id == d.id))
    else {
        keepsakes_page(ui, lang, data, (layout, storeroom), assets);
        return;
    };
    ui.heading(&lang.decor(&d.id));
    let root = ui.root();
    ui.menu_option(root, &lang.t("menu.back"), Action::BookBack, false);
    let header = ui.node(
        root,
        Node {
            column_gap: px(14),
            align_items: AlignItems::Center,
            ..default()
        },
    );
    let icon = format!("sprites/decor/{}.png", d.id);
    if asset_exists(&icon) {
        ui.image_sized(header, assets.load(icon), Vec2::splat(KEEPSAKE_PICTURE));
    }
    let notes = ui.node(
        header,
        Node {
            flex_direction: FlexDirection::Column,
            flex_shrink: 1.0,
            row_gap: px(4),
            ..default()
        },
    );
    let place = if placed(layout, &d.id) {
        "book.keepsake_placed"
    } else {
        "book.keepsake_stored"
    };
    if let Some((story, chapter)) = keepsake_origin(data, &d.id) {
        let ghost = &data.stories[story].ghost;
        ui.add_text(
            notes,
            &lang.f(
                "book.keepsake_from",
                &[("ghost", &lang.ghost(ghost)), ("chapter", &chapter)],
            ),
            true,
            15.0,
            TEXT,
        );
        ui.add_text(notes, &lang.t(place), false, 13.0, crate::ui::TEXT_DIM);
        ui.text(&lang.f(
            "quote",
            &[("text", &lang.t(&format!("story.{ghost}.{chapter}.thanks")))],
        ));
        // A logbook keeps one entry per chapter finished so far; later
        // chapters stay unwritten.
        if data.stories[story].logbook.as_deref() == Some(&d.id) {
            ui.section(&lang.t("book.log_title"));
            let done = progress
                .chapter(story)
                .min(data.stories[story].chapters.len());
            let nights = journal.chapter_nights.get(&story);
            for n in 1..=done {
                let text = lang.t(&format!("story.{ghost}.{n}.log"));
                match nights.and_then(|nights| nights.get(n - 1)) {
                    Some(night) => {
                        ui.text(&lang.f("book.log_entry", &[("night", night), ("text", &text)]))
                    }
                    None => ui.text(&text),
                };
            }
        }
        if let Some(i) = data.ghost_index(ghost).filter(|_| journal.has_seen(ghost)) {
            ui.menu_option(root, &lang.ghost(ghost), Action::GhostProfile(i), false);
        }
    } else {
        ui.add_text(notes, &lang.t(place), false, 13.0, crate::ui::TEXT_DIM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{ActionButton, UiFont};

    #[test]
    fn index_links_and_illustrated_pages_require_an_actual_sighting() {
        #[derive(Resource, Default)]
        struct Requested(Option<usize>);

        let data = GameData::load();
        let shy = data.ghost_index("shy").unwrap();
        let shade = data.ghost_index("shade").unwrap();
        let mut progress = crate::day_cycle::Progress::new(0);
        progress.journal.met("shy", "lavender");
        let lang = Lang::load("en");
        let shade_name = lang.ghost("shade");
        let mut app = App::new();
        let root = app.world_mut().spawn_empty().id();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .insert_resource(lang)
            .init_resource::<UiFont>()
            .init_resource::<Requested>()
            .insert_resource(data)
            .insert_resource(progress)
            .add_systems(
                Update,
                move |mut commands: Commands,
                      font: Res<UiFont>,
                      lang: Res<Lang>,
                      data: Res<GameData>,
                      assets: Res<AssetServer>,
                      progress: Res<crate::day_cycle::Progress>,
                      requested: Res<Requested>| {
                    let mut ui = PanelBuilder::new(&mut commands, root, &font);
                    match requested.0 {
                        Some(i) => {
                            ghost_profile(&mut ui, &lang, &data, &progress.journal, i, &assets)
                        }
                        None => book_page(&mut ui, &lang, &data, &progress.journal),
                    }
                },
            );
        for requested in [None, Some(shade)] {
            app.world_mut().resource_mut::<Requested>().0 = requested;
            app.update();
            let world = app.world_mut();
            let cards: Vec<_> = world
                .query::<&ActionButton>()
                .iter(world)
                .filter_map(|button| {
                    if let Action::GhostProfile(id) = &button.0 {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(cards, [shy]);
            assert!(
                !world
                    .query::<&Text>()
                    .iter(world)
                    .any(|line| line.0.contains(&shade_name))
            );
        }
        app.world_mut()
            .resource_mut::<crate::day_cycle::Progress>()
            .journal
            .met("shade", "");
        app.update();
        let world = app.world_mut();
        assert!(
            world
                .query::<&Text>()
                .iter(world)
                .any(|line| line.0 == shade_name)
        );
        let portrait = world
            .resource::<AssetServer>()
            .load::<Image>("sprites/ghosts/shade.png");
        assert!(
            world
                .query::<&ImageNode>()
                .iter(world)
                .any(|image| image.image == portrait)
        );
    }

    #[test]
    fn a_logbook_has_an_entry_for_every_chapter_and_records_its_night() {
        let data = GameData::load();
        let lang = Lang::load("en");
        let mut found = false;
        for (i, story) in data.stories.iter().enumerate() {
            let Some(book) = &story.logbook else { continue };
            found = true;
            assert_eq!(
                keepsake_origin(&data, book).map(|(s, _)| s),
                Some(i),
                "{book} is left by {}",
                story.ghost
            );
            for n in 1..=story.chapters.len() {
                let key = format!("story.{}.{n}.log", story.ghost);
                assert_ne!(lang.t(&key), key, "missing {key}");
            }
            let mut progress = crate::day_cycle::Progress::new(0);
            progress.night = 7;
            progress.finish_chapter(i);
            progress.night = 9;
            progress.finish_chapter(i);
            assert_eq!(progress.journal.chapter_nights[&i], [7, 9]);
        }
        assert!(found, "Ida's logbook keeps the lighthouse log");
    }

    #[test]
    fn the_book_remembers_each_wish_once() {
        let mut j = Journal::default();
        j.met("shy", "lavender");
        j.met("shy", "lavender");
        j.met("shy", "tea_tin");
        j.served("shy", "lavender", None);
        let e = &j.book["shy"];
        assert_eq!((e.met, e.served), (3, 1));
        assert_eq!(e.wishes, ["lavender", "tea_tin"]);
    }
}
