//! Ghost currency, purses and the cash drawer.
//!
//! All amounts are counted internally in "button units" (button = 1).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;
use crate::rng::Rng;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Coin {
    Button,
    Candle,
    Spoon,
}

impl Coin {
    /// Largest coin first – handy for greedy algorithms.
    pub const DESCENDING: [Coin; 3] = [Coin::Spoon, Coin::Candle, Coin::Button];
    pub const ASCENDING: [Coin; 3] = [Coin::Button, Coin::Candle, Coin::Spoon];

    pub fn value(self) -> u32 {
        match self {
            Coin::Button => 1,
            Coin::Candle => 5,
            Coin::Spoon => 25,
        }
    }

    /// File and catalog id ("button", "candle", "spoon").
    pub fn id(self) -> &'static str {
        match self {
            Coin::Button => "button",
            Coin::Candle => "candle",
            Coin::Spoon => "spoon",
        }
    }

    pub fn name(self, lang: &Lang) -> String {
        lang.t(match self {
            Coin::Button => "coin.button",
            Coin::Candle => "coin.candle",
            Coin::Spoon => "coin.spoon",
        })
    }
}

/// A set of coins – a ghost's purse, the cash drawer, change …
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Purse {
    pub buttons: u32,
    pub candles: u32,
    pub spoons: u32,
}

impl Purse {
    pub fn get(&self, coin: Coin) -> u32 {
        match coin {
            Coin::Button => self.buttons,
            Coin::Candle => self.candles,
            Coin::Spoon => self.spoons,
        }
    }

    fn get_mut(&mut self, coin: Coin) -> &mut u32 {
        match coin {
            Coin::Button => &mut self.buttons,
            Coin::Candle => &mut self.candles,
            Coin::Spoon => &mut self.spoons,
        }
    }

    pub fn add(&mut self, coin: Coin, count: u32) {
        *self.get_mut(coin) += count;
    }

    pub fn total(&self) -> u32 {
        Coin::ASCENDING
            .iter()
            .map(|c| c.value() * self.get(*c))
            .sum()
    }

    pub fn merge(&mut self, other: &Purse) {
        for c in Coin::ASCENDING {
            self.add(c, other.get(c));
        }
    }

    pub fn contains(&self, other: &Purse) -> bool {
        Coin::ASCENDING
            .iter()
            .all(|c| self.get(*c) >= other.get(*c))
    }

    /// Removes exactly these coins. Returns `false` (and changes nothing)
    /// if there are not enough.
    pub fn remove(&mut self, other: &Purse) -> bool {
        if !self.contains(other) {
            return false;
        }
        for c in Coin::ASCENDING {
            *self.get_mut(c) -= other.get(c);
        }
        true
    }

    /// Fewest coins for an amount (unlimited supply).
    pub fn greedy(mut amount: u32) -> Purse {
        let mut p = Purse::default();
        for c in Coin::DESCENDING {
            let n = amount / c.value();
            p.add(c, n);
            amount -= n * c.value();
        }
        p
    }

    /// Pays an amount out of this purse, e.g. to the ferryman or a trader,
    /// the way a shopkeeper would: with the largest coins that cover it,
    /// taking change back (in the fewest coins). That keeps the buttons and
    /// candle stubs the register needs for giving change.
    pub fn pay(&mut self, amount: u32) -> bool {
        if self.total() < amount {
            return false;
        }
        let mut remaining = amount as i64;
        for c in Coin::DESCENDING {
            if remaining <= 0 {
                break;
            }
            let value = c.value() as i64;
            let needed = (remaining + value - 1) / value;
            let used = needed.min(self.get(c) as i64);
            *self.get_mut(c) -= used as u32;
            remaining -= used * value;
        }
        self.merge(&Purse::greedy((-remaining).max(0) as u32));
        true
    }

    /// Human-readable description, e.g. "silver spoon ×2, button ×1". The
    /// "×" form avoids plurals, which differ wildly between languages.
    pub fn describe(&self, lang: &Lang) -> String {
        let parts: Vec<String> = Coin::DESCENDING
            .iter()
            .filter(|c| self.get(**c) > 0)
            .map(|c| {
                lang.f(
                    "purse.entry",
                    &[("coin", &c.name(lang)), ("count", &self.get(*c))],
                )
            })
            .collect();
        if parts.is_empty() {
            lang.t("purse.nothing")
        } else {
            lang.join(&parts)
        }
    }
}

/// The shop's cash drawer.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct Drawer(pub Purse);

/// How a ghost pays: usually with one kind of coin that requires change,
/// sometimes the exact amount.
pub fn ghost_payment(price: u32, allowed: &[Coin], rng: &mut Rng) -> Purse {
    let allowed: Vec<Coin> = if allowed.is_empty() {
        Coin::ASCENDING.to_vec()
    } else {
        allowed.to_vec()
    };

    // Exact payment is only possible when buttons are allowed.
    if allowed.contains(&Coin::Button) && rng.chance(0.25) {
        let exact = Purse::greedy(price);
        if Coin::ASCENDING
            .iter()
            .all(|c| exact.get(*c) == 0 || allowed.contains(c))
        {
            return exact;
        }
    }

    let sensible: Vec<Coin> = allowed
        .iter()
        .copied()
        .filter(|c| price.div_ceil(c.value()) <= 8)
        .collect();
    let coin = if sensible.is_empty() {
        *allowed.iter().max_by_key(|c| c.value()).unwrap()
    } else {
        sensible[rng.index(sensible.len())]
    };
    let mut p = Purse::default();
    p.add(coin, price.div_ceil(coin.value()).max(1));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn purse(spoons: u32, candles: u32, buttons: u32) -> Purse {
        Purse {
            buttons,
            candles,
            spoons,
        }
    }

    #[test]
    fn greedy_change() {
        assert_eq!(Purse::greedy(37), purse(1, 2, 2));
        assert_eq!(Purse::greedy(0), Purse::default());
    }

    #[test]
    fn pay_with_change() {
        let mut p = purse(1, 0, 0);
        assert!(p.pay(7));
        assert_eq!(p.total(), 18);
        assert_eq!(p, purse(0, 3, 3));
    }

    #[test]
    fn pay_exact_and_insufficient() {
        let mut p = purse(0, 2, 3);
        assert!(p.pay(13));
        assert_eq!(p.total(), 0);
        let mut p = purse(0, 1, 0);
        assert!(!p.pay(6));
        assert_eq!(p, purse(0, 1, 0));
    }

    #[test]
    fn paying_keeps_the_small_coins() {
        // A crate for 12: one silver spoon, 13 back in candle stubs and buttons.
        let mut p = purse(2, 6, 12);
        assert!(p.pay(12));
        assert_eq!(p, purse(1, 8, 15));
        // Without spoons the candle stubs go first.
        let mut p = purse(0, 3, 4);
        assert!(p.pay(12));
        assert_eq!(p, purse(0, 0, 7));
    }

    #[test]
    fn pay_mixed_shortfall() {
        // 2 buttons + 1 candle stub, price 4 -> pay 2 buttons + candle, get 3 back
        let mut p = purse(0, 1, 2);
        assert!(p.pay(4));
        assert_eq!(p.total(), 3);
    }

    #[test]
    fn ghost_always_pays_enough() {
        let mut rng = Rng::new(42);
        for price in 1..80 {
            for allowed in [&[][..], &[Coin::Button][..], &[Coin::Spoon][..]] {
                let p = ghost_payment(price, allowed, &mut rng);
                assert!(p.total() >= price, "price {price}, paid {p:?}");
            }
        }
    }
}
