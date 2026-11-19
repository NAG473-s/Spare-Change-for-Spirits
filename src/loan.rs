//! The bankruptcy rule. A shop that has neither the coins nor the wares to
//! get going again is not the end of the game: in the evening the ferryman
//! lends enough for a fresh start and takes it back, with a surcharge, out
//! of the drawer at dawn. Whatever the drawer cannot cover stays owed and
//! is taken at the next dawn.

use bevy::prelude::*;
use serde::Deserialize;

use crate::accounts::{Accounts, open_books};
use crate::data::GameData;
use crate::day_cycle::{Phase, Progress, start_morning};
use crate::economy::{Drawer, Purse};
use crate::ferry::sale_price;
use crate::i18n::Lang;
use crate::inventory::{CratesOrdered, Stock};
use crate::ui::GameLog;

pub struct LoanPlugin;

impl Plugin for LoanPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loan>()
            .add_systems(OnEnter(Phase::Evening), lend.after(open_books))
            .add_systems(OnEnter(Phase::Morning), repay.after(start_morning));
    }
}

/// The ferryman's terms (`assets/data/config.ron`).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct LoanTerms {
    /// What he lends, in button units.
    pub amount: u32,
    /// Surcharge on top, in percent of the amount.
    pub surcharge_percent: u32,
}

impl LoanTerms {
    /// What has to be paid back for one loan.
    pub fn due(&self) -> u32 {
        (self.amount * (100 + self.surcharge_percent)).div_ceil(100)
    }
}

/// What the shop still owes the ferryman.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loan {
    pub owed: u32,
}

/// The shop is stuck when the drawer and everything in stock, sold at
/// tonight's prices, would not even pay for the cheapest crate on offer.
pub fn is_stuck(data: &GameData, night: u32, drawer: u32, stock: &Stock) -> bool {
    let Some(cheapest) = data
        .products
        .iter()
        .filter(|p| p.unlock_night <= night)
        .map(|p| p.crate_cost)
        .min()
    else {
        return false;
    };
    let wares: u32 = stock
        .units()
        .iter()
        .map(|(id, ages)| sale_price(data, id, night) * ages.len() as u32)
        .sum();
    data.crates_tonight(night) > 0 && drawer + wares < cheapest
}

/// Evening: a stuck shop gets the ferryman's loan (once per evening – a
/// game continued at this evening already has the coins).
#[allow(clippy::too_many_arguments)]
pub fn lend(
    data: Res<GameData>,
    progress: Res<Progress>,
    stock: Res<Stock>,
    crates: Res<CratesOrdered>,
    lang: Res<Lang>,
    mut drawer: ResMut<Drawer>,
    mut loan: ResMut<Loan>,
    mut accounts: ResMut<Accounts>,
    mut log: ResMut<GameLog>,
) {
    let night = progress.night;
    if crates.0 > 0 || !is_stuck(&data, night, drawer.total(), &stock) {
        return;
    }
    let terms = data.config.ferryman_loan;
    drawer.merge(&Purse::greedy(terms.amount));
    loan.owed += terms.due();
    accounts.borrowed += terms.amount;
    accounts.night_start = drawer.total();
    log.push(lang.f(
        "log.ferryman_loan",
        &[("amount", &terms.amount), ("due", &loan.owed)],
    ));
}

/// Dawn: the ferryman takes back what the drawer can cover.
pub fn repay(mut drawer: ResMut<Drawer>, mut loan: ResMut<Loan>, mut accounts: ResMut<Accounts>) {
    let amount = loan.owed.min(drawer.total());
    if amount > 0 && drawer.pay(amount) {
        loan.owed -= amount;
        accounts.repaid += amount;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn surcharge_rounds_up() {
        let terms = LoanTerms {
            amount: 20,
            surcharge_percent: 25,
        };
        assert_eq!(terms.due(), 25);
        let terms = LoanTerms {
            amount: 9,
            surcharge_percent: 10,
        };
        assert_eq!(terms.due(), 10);
    }

    #[test]
    fn stuck_only_without_coins_and_wares() {
        let data = GameData::load();
        let cheapest = data
            .products
            .iter()
            .filter(|p| p.unlock_night <= 1)
            .map(|p| p.crate_cost)
            .min()
            .unwrap();
        let empty = Stock::default();
        assert!(is_stuck(&data, 1, cheapest - 1, &empty));
        assert!(!is_stuck(&data, 1, cheapest, &empty));
        // Wares worth enough to sell keep the shop going.
        let id = data.products[0].id.clone();
        let stock = Stock::from_units(BTreeMap::from([(id, vec![0; 20])]));
        assert!(!is_stuck(&data, 1, 0, &stock));
    }
}
