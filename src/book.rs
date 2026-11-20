//! The ghost book: a page for every kind of ghost the shop has met, filled in
//! while playing – how often it came and was served, what it likes, how it
//! pays and which wishes it has whispered so far – with the regulars, Hedda's
//! letters and the keepsakes. Opened from the pause menu or the evening panel.
//!
//! The book lives in the save as part of [`Journal`], together with the
//! regulars (`regulars.rs`) and the ware in the shop window (`showcase.rs`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::day_cycle::Progress;
use crate::i18n::Lang;
use crate::shop::{ShopLayout, Storeroom};
use crate::ui::{Action, PanelBuilder};
use crate::upgrades::Upgrades;

/// What the shop remembers about its customers (saved with the game).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Journal {
    /// One entry per kind of ghost met (ghost id).
    pub book: BTreeMap<String, BookEntry>,
    /// Ghosts that come back by name.
    pub regulars: Vec<Regular>,
    /// The ware shown in the shop window (product id).
    pub showcase: Option<String>,
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
}

impl Regular {
    pub fn name(&self, lang: &Lang) -> String {
        lang.t(&format!("regular.name.{}", self.name))
    }
}

impl Journal {
    /// A ghost came in, wishing for `wish` (empty for a fare).
    pub fn met(&mut self, ghost: &str, wish: &str) {
        let entry = self.book.entry(ghost.to_string()).or_default();
        entry.met += 1;
        if !wish.is_empty() && !entry.wishes.iter().any(|w| w == wish) {
            entry.wishes.push(wish.to_string());
        }
    }

    /// A ghost was sold something.
    pub fn served(&mut self, ghost: &str) {
        self.book.entry(ghost.to_string()).or_default().served += 1;
    }

    pub fn regular(&self, ghost: &str) -> Option<&Regular> {
        self.regulars.iter().find(|r| r.ghost == ghost)
    }
}

/// The ghosts the book has a page for: customers and story ghosts, but not
/// the shade and not Hedda.
fn in_book(data: &GameData) -> impl Iterator<Item = &crate::data::GhostDef> {
    data.ghosts.iter().filter(|g| {
        g.id != crate::ghosts::THIEF_GHOST
            && !data
                .stories
                .iter()
                .any(|s| s.finale_guest.as_deref() == Some(&g.id))
    })
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

/// The book's page in the pause menu.
pub fn book_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    (layout, storeroom, upgrades): (&ShopLayout, &Storeroom, &Upgrades),
) {
    let journal = &progress.journal;
    let ghosts: Vec<_> = in_book(data).collect();
    let met: Vec<_> = ghosts
        .iter()
        .filter(|g| journal.book.contains_key(&g.id))
        .collect();
    ui.heading(&lang.f(
        "book.title",
        &[("met", &met.len()), ("total", &ghosts.len())],
    ));
    let root = ui.root();
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSaveMenu, false);
    if met.is_empty() {
        ui.dim(&lang.t("book.empty"));
    }
    for g in &met {
        let entry = &journal.book[&g.id];
        ui.section(&lang.f(
            "book.entry",
            &[
                ("ghost", &lang.ghost(&g.id)),
                ("met", &entry.met),
                ("served", &entry.served),
            ],
        ));
        if let Some(r) = journal.regular(&g.id) {
            ui.text(&lang.f(
                "book.regular",
                &[
                    ("name", &r.name(lang)),
                    ("visits", &r.visits),
                    ("item", &lang.product(&r.favourite)),
                ],
            ));
        }
        let mut about: Vec<String> = g.needs.iter().map(|n| n.describe(lang)).collect();
        if !g.pays_with.is_empty() {
            let coins: Vec<String> = g.pays_with.iter().map(|c| c.name(lang)).collect();
            about.push(lang.f("ghost_tip.pays_with", &[("coins", &lang.join(&coins))]));
        }
        let percent = ((g.pay_multiplier - 1.0) * 100.0).round() as i32;
        if percent > 0 {
            about.push(lang.f("ghost_tip.pays_more", &[("percent", &percent)]));
        } else if percent < 0 {
            about.push(lang.f("ghost_tip.pays_less", &[("percent", &-percent)]));
        }
        for (chance, key) in [
            (g.quirks.knock_chance, "ghost_tip.knocks"),
            (g.quirks.counterfeit_chance, "ghost_tip.counterfeit"),
            (g.quirks.double_pay_chance, "ghost_tip.double_pay"),
        ] {
            if chance > 0.0 {
                about.push(lang.t(key));
            }
        }
        if !about.is_empty() {
            ui.dim(&about.join(" · "));
        }
        let wishes: Vec<String> = entry.wishes.iter().map(|w| lang.product(w)).collect();
        if !wishes.is_empty() {
            ui.text(&lang.f(
                "book.wishes",
                &[
                    ("items", &lang.join(&wishes)),
                    ("seen", &entry.wishes.len()),
                    ("total", &g.wishes.len().max(entry.wishes.len())),
                ],
            ));
        }
    }
    let unmet = ghosts.len() - met.len();
    if unmet > 0 {
        ui.dim(&lang.f("book.unmet", &[("count", &unmet)]));
    }

    ui.section(&lang.t("book.letters"));
    for n in letters_found(data, upgrades) {
        ui.dim(&lang.t(&format!("letter.{n}")));
    }

    let keepsakes: Vec<String> = data
        .decor
        .iter()
        .filter(|d| d.keepsake)
        .filter(|d| {
            storeroom.count(&d.id) > 0
                || layout
                    .cells
                    .iter()
                    .any(|c| c.decor.as_deref() == Some(&d.id))
        })
        .map(|d| lang.decor(&d.id))
        .collect();
    ui.section(&lang.t("book.keepsakes"));
    if keepsakes.is_empty() {
        ui.dim(&lang.t("book.no_keepsakes"));
    } else {
        ui.text(&lang.join(&keepsakes));
    }

    let root = ui.root();
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSaveMenu, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_book_remembers_each_wish_once() {
        let mut j = Journal::default();
        j.met("shy", "lavender");
        j.met("shy", "lavender");
        j.met("shy", "tea_tin");
        j.served("shy");
        let e = &j.book["shy"];
        assert_eq!((e.met, e.served), (3, 1));
        assert_eq!(e.wishes, ["lavender", "tea_tin"]);
    }
}
