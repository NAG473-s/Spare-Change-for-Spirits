//! Hedda's first sale uses the same crate, shelf, customer and register as the rest of the game.
use serde::{Deserialize, Serialize};

use crate::day_cycle::Progress;
use crate::economy::{Coin, Purse};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::shop::ShopLayout;
use crate::ui::{Action, PanelBuilder};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FirstSale {
    Following,
    // Profiles created before the guide keep their original night.
    #[default]
    Skipped,
    Complete,
}

/// A first-night save starts from the paid-for crates and replays the one reserved unit.
/// It contains no world entity IDs, so the counter can be rebuilt after loading.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FirstSaleCheckpoint {
    Wish,
    AfterSale(Box<crate::day_cycle::NightReport>),
    Change {
        coins: Vec<crate::register::PaidCoin>,
        given: Purse,
        recount: bool,
    },
}

impl Progress {
    pub fn guiding_first_sale(&self) -> bool {
        self.night == 1 && self.journal.first_sale == FirstSale::Following
    }
}

impl FirstSaleCheckpoint {
    pub fn settled_report(&self) -> Option<&crate::day_cycle::NightReport> {
        if let Self::AfterSale(report) = self {
            Some(report)
        } else {
            None
        }
    }
}

pub fn book_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    progress: &Progress,
    phase: crate::day_cycle::Phase,
) {
    ui.heading(&lang.t("guide_title"));
    for step in [
        "guide_buy",
        "guide_shelf",
        "guide_open",
        "guide_wish",
        "guide_change",
    ] {
        ui.text(&lang.t(step));
    }
    if phase == crate::day_cycle::Phase::Evening
        && progress.night == 1
        && progress.journal.first_sale == FirstSale::Skipped
    {
        ui.primary(&lang.t("guide_resume"), Action::ResumeGuide);
    }
}

/// The first customer asks for a real unit the player chose to put on display.
pub fn displayed_ware<'a>(layout: &'a ShopLayout, stock: &Stock) -> Option<&'a str> {
    layout
        .cells
        .iter()
        .flat_map(|c| &c.products)
        .find(|p| stock.count(p) > 0)
        .map(String::as_str)
}

pub fn evening_step(layout: &ShopLayout, stock: &Stock) -> &'static str {
    if displayed_ware(layout, stock).is_some() {
        "guide_open"
    } else if stock.units().values().any(|units| !units.is_empty()) {
        "guide_shelf"
    } else {
        "guide_buy"
    }
}

pub fn prompt(ui: &mut PanelBuilder, lang: &Lang, step: &str) {
    ui.section(&lang.t("guide_title"));
    ui.text(&lang.t(step));
    ui.button_in(
        ui.root(),
        &lang.t("guide_skip"),
        Action::SkipGuide,
        false,
        true,
    );
}

/// One extra button always makes change possible, even when the ferry emptied the drawer.
pub fn payment(price: u32, allowed: &[Coin]) -> Purse {
    let mut purse = if allowed == [Coin::Button] {
        Purse {
            buttons: price,
            ..Default::default()
        }
    } else {
        Purse::greedy(price)
    };
    purse.add(Coin::Button, 1);
    purse
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn following_the_normal_ferry_and_shelf_actions_advances_or_reverses_the_guide() {
        let mut layout = ShopLayout::new(10, 5);
        let mut stock = Stock::default();
        assert_eq!(evening_step(&layout, &stock), "guide_buy");
        let ware = "powder_compact";
        stock.add(ware, 2);
        assert_eq!(evening_step(&layout, &stock), "guide_shelf");
        layout.cells[12].products.push(ware.into());
        assert_eq!(evening_step(&layout, &stock), "guide_open");
        assert_eq!(displayed_ware(&layout, &stock), Some(ware));
        assert!(stock.remove_new(ware, 2));
        assert_eq!(evening_step(&layout, &stock), "guide_buy");
    }
}
