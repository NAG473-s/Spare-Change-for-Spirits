//! Counter and register: guess the wish, take payment, inspect coins,
//! hand out change – and decide what to do when a ghost pays twice.

use bevy::prelude::*;

use crate::data::{Config, GameData};
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::{Coin, Drawer, Purse, ghost_payment};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::inventory::Stock;
use crate::rng::Rng;
use crate::shop::Keepsakes;
use crate::ui::{Action, GameLog};

pub struct RegisterPlugin;

impl Plugin for RegisterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Register>()
            .add_systems(OnExit(Phase::Night), |mut r: ResMut<Register>| {
                *r = Register::default()
            })
            .add_systems(
                Update,
                (serve_front_ghost, make_change, answer_double_pay).run_if(in_state(Phase::Night)),
            );
    }
}

/// One coin the ghost put on the counter.
#[derive(Clone, Debug)]
pub struct PaidCoin {
    pub coin: Coin,
    /// How the coin looks or feels – counterfeits give themselves away here.
    pub tell: String,
    pub fake: bool,
    /// The player has already inspected this coin.
    pub checked: bool,
}

/// A sale in progress at the register.
#[derive(Clone, Debug)]
pub struct Sale {
    pub ghost: Entity,
    pub product: String,
    pub price: u32,
    /// What the ghost put on the counter (already in the cash drawer).
    pub coins: Vec<PaidCoin>,
    /// What the player has counted out as change so far.
    pub given: Purse,
    /// Story chapter this sale completes, if it is the story ghost.
    pub chapter: Option<usize>,
}

impl Sale {
    pub fn paid(&self) -> Purse {
        let mut p = Purse::default();
        for c in &self.coins {
            p.add(c.coin, 1);
        }
        p
    }

    pub fn change_due(&self) -> u32 {
        self.paid().total().saturating_sub(self.price)
    }
}

/// A forgetful ghost offers to pay a second time.
#[derive(Clone, Debug)]
pub struct DoublePay {
    pub ghost: Entity,
    pub amount: u32,
}

#[derive(Resource, Default)]
pub struct Register {
    pub sale: Option<Sale>,
    pub question: Option<DoublePay>,
}

impl Register {
    pub fn is_busy(&self) -> bool {
        self.sale.is_some() || self.question.is_some()
    }

    pub fn is_busy_with(&self, ghost: Entity) -> bool {
        self.sale.as_ref().is_some_and(|s| s.ghost == ghost)
            || self.question.as_ref().is_some_and(|q| q.ghost == ghost)
    }
}

/// Lays the coins of a payment out on the counter, one by one, each with a
/// description. With `counterfeit`, one of them is fake.
pub fn paid_coins(
    purse: &Purse,
    counterfeit: bool,
    config: &Config,
    rng: &mut Rng,
) -> Vec<PaidCoin> {
    let mut coins = Vec::new();
    for coin in Coin::DESCENDING {
        for _ in 0..purse.get(coin) {
            let tells = &config.coin_tells_genuine;
            coins.push(PaidCoin {
                coin,
                tell: tells[rng.index(tells.len())].clone(),
                fake: false,
                checked: false,
            });
        }
    }
    if counterfeit && !coins.is_empty() {
        let i = rng.index(coins.len());
        let tells = &config.coin_tells_fake;
        coins[i].fake = true;
        coins[i].tell = tells[rng.index(tells.len())].clone();
    }
    coins
}

/// The player hands an item to the ghost at the front – or declines.
fn serve_front_ghost(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    stock: Res<Stock>,
    mut register: ResMut<Register>,
    mut queue: ResMut<Queue>,
    mut drawer: ResMut<Drawer>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut ghosts: Query<&mut Ghost>,
) {
    for action in actions.read() {
        if register.is_busy() {
            continue;
        }
        let Some(front) = queue.front() else { continue };
        let Ok(mut ghost) = ghosts.get_mut(front) else {
            continue;
        };
        if ghost.state != GhostState::Waiting {
            continue;
        }
        let def = &data.ghosts[ghost.def];

        match action {
            Action::Offer(product) => {
                if stock.count(product) == 0 {
                    continue;
                }
                if *product != ghost.wish {
                    log.push(format!(
                        "\"That … is not it.\" {} drifts away, disappointed.",
                        def.the_cap()
                    ));
                    ghost.state = GhostState::Leaving;
                    queue.remove(front);
                    progress.reputation -= 1;
                    report.unhappy += 1;
                    continue;
                }
                let base = data.product(product).map_or(1, |p| p.price);
                let price = ((base as f32 * def.pay_multiplier).round() as u32).max(1);
                let purse = ghost_payment(price, &def.pays_with, &mut rng);
                let counterfeit = rng.chance(def.quirks.counterfeit_chance);
                let coins = paid_coins(&purse, counterfeit, &data.config, &mut rng);
                // Payment goes straight into the drawer – change is taken from there too.
                drawer.merge(&purse);
                ghost.state = GhostState::AtRegister;
                log.push(format!(
                    "{} places {} on the counter.",
                    def.the_cap(),
                    purse.describe()
                ));
                register.sale = Some(Sale {
                    ghost: front,
                    product: product.clone(),
                    price,
                    coins,
                    given: Purse::default(),
                    chapter: ghost.chapter,
                });
            }
            Action::Decline => {
                log.push(format!(
                    "You apologise. {} nods silently and leaves.",
                    def.the_cap()
                ));
                ghost.state = GhostState::Leaving;
                queue.remove(front);
            }
            _ => {}
        }
    }
}

/// Inspect coins, count out change and hand it over.
fn make_change(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut register: ResMut<Register>,
    mut drawer: ResMut<Drawer>,
    mut stock: ResMut<Stock>,
    mut queue: ResMut<Queue>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut keepsakes: ResMut<Keepsakes>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut ghosts: Query<&mut Ghost>,
) {
    for action in actions.read() {
        let Some(sale) = register.sale.as_mut() else {
            continue;
        };
        match action {
            Action::CheckCoin(i) => {
                let Some(paid) = sale.coins.get_mut(*i) else {
                    continue;
                };
                if paid.checked {
                    continue;
                }
                paid.checked = true;
                let name = ghosts
                    .get(sale.ghost)
                    .map(|g| data.ghosts[g.def].the_cap())
                    .unwrap_or_default();
                if paid.fake {
                    // The ghost swaps it for a genuine one of the same kind,
                    // so the drawer total stays as it is.
                    paid.fake = false;
                    paid.tell = "genuine (swapped)".into();
                    report.counterfeits_caught += 1;
                    log.push(format!(
                        "You hold the {} up to the candle – a fake! {name} cackles and swaps it.",
                        paid.coin.name(1)
                    ));
                } else {
                    progress.reputation -= 1;
                    log.push(format!(
                        "The {} is genuine. {name} looks deeply offended.",
                        paid.coin.name(1)
                    ));
                }
            }
            Action::AddChange(coin) => {
                if sale.given.get(*coin) < drawer.get(*coin) {
                    sale.given.add(*coin, 1);
                }
            }
            Action::ResetChange => sale.given = Purse::default(),
            Action::GiveChange => {
                let sale = register.sale.take().expect("checked above");
                drawer.remove(&sale.given);
                stock.take_one(&sale.product);
                report.sales += 1;
                report.revenue += sale.price;
                report
                    .counterfeits_taken
                    .extend(sale.coins.iter().filter(|c| c.fake).map(|c| c.coin));

                let due = sale.change_due();
                let given = sale.given.total();
                let Ok(mut ghost) = ghosts.get_mut(sale.ghost) else {
                    queue.remove(sale.ghost);
                    continue;
                };
                let def = &data.ghosts[ghost.def];
                let name = def.the_cap();
                let correct = given == due;

                if given < due {
                    report.change_wrong += 1;
                    progress.reputation -= 2;
                    log.push(format!(
                        "Too little change ({given} instead of {due}). {name} feels cheated."
                    ));
                } else if given > due {
                    report.change_wrong += 1;
                    log.push(format!(
                        "Too much change ({given} instead of {due}). {name} giggles softly."
                    ));
                } else {
                    report.change_correct += 1;
                    progress.reputation += 1;
                }

                if let Some(chapter) = sale.chapter {
                    // The story ghost got what he needed, whatever the change.
                    let story = &data.story;
                    let c = &story.chapters[chapter];
                    log.push(format!("{}: \"{}\"", story.name, c.thanks));
                    report.story_line = Some(c.thanks.clone());
                    if let Some(k) = &c.keepsake {
                        keepsakes.give(k);
                        report.keepsakes.push(k.clone());
                    }
                    progress.story_chapter = chapter + 1;
                    if progress.story_chapter >= story.chapters.len() {
                        log.push(story.ending.clone());
                        report.redeemed.push(story.name.clone());
                        ghost.state = GhostState::Ascending;
                        ghost.timer = 0.0;
                    } else {
                        ghost.state = GhostState::Leaving;
                    }
                } else if !correct {
                    ghost.state = GhostState::Leaving;
                } else if rng.chance(def.quirks.double_pay_chance) {
                    log.push(format!(
                        "{name} blinks. \"Oh – did I pay already? Here …\" It holds out {} again.",
                        sale.price
                    ));
                    register.question = Some(DoublePay {
                        ghost: sale.ghost,
                        amount: sale.price,
                    });
                } else {
                    ghost.state = GhostState::Ascending;
                    ghost.timer = 0.0;
                    report.redeemed.push(def.name.clone());
                    log.push(format!(
                        "Exactly right! {name} clutches the {} and dissolves into warm light.",
                        data.product_name(&sale.product)
                    ));
                }
                // A ghost that wants to pay again keeps its place at the counter.
                if register.question.is_none() {
                    queue.remove(sale.ghost);
                }
            }
            _ => {}
        }
    }
}

/// Keep the second payment of a forgetful ghost – or tell it the truth.
fn answer_double_pay(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut register: ResMut<Register>,
    mut queue: ResMut<Queue>,
    mut drawer: ResMut<Drawer>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut ghosts: Query<&mut Ghost>,
) {
    for action in actions.read() {
        let keep = match action {
            Action::KeepSecondPayment => true,
            Action::RefuseSecondPayment => false,
            _ => continue,
        };
        let Some(question) = register.question.take() else {
            continue;
        };
        queue.remove(question.ghost);
        let Ok(mut ghost) = ghosts.get_mut(question.ghost) else {
            continue;
        };
        let def = &data.ghosts[ghost.def];
        let name = def.the_cap();
        if keep {
            drawer.merge(&Purse::greedy(question.amount));
            progress.reputation -= 1;
            report.dishonest += 1;
            ghost.state = GhostState::Leaving;
            log.push(format!(
                "You pocket the second payment. {name} drifts off, still confused."
            ));
        } else {
            progress.reputation += 2;
            report.honest += 1;
            report.redeemed.push(def.name.clone());
            ghost.state = GhostState::Ascending;
            ghost.timer = 0.0;
            log.push(format!(
                "\"You already paid.\" {name} smiles, relieved, and dissolves into warm light."
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        crate::data::GameData::load().config
    }

    #[test]
    fn paid_coins_keep_the_payment_and_fake_at_most_one() {
        let config = config();
        let mut rng = Rng::new(7);
        let purse = Purse {
            buttons: 2,
            candles: 1,
            spoons: 1,
        };
        for counterfeit in [false, true] {
            let coins = paid_coins(&purse, counterfeit, &config, &mut rng);
            assert_eq!(coins.len(), 4);
            let mut total = Purse::default();
            for c in &coins {
                total.add(c.coin, 1);
            }
            assert_eq!(total, purse);
            let fakes: Vec<_> = coins.iter().filter(|c| c.fake).collect();
            assert_eq!(fakes.len(), usize::from(counterfeit));
            for f in fakes {
                assert!(config.coin_tells_fake.contains(&f.tell));
            }
        }
    }

    #[test]
    fn change_due_counts_every_coin() {
        let spoon = Purse {
            buttons: 0,
            candles: 0,
            spoons: 1,
        };
        let sale = Sale {
            ghost: Entity::PLACEHOLDER,
            product: "x".into(),
            price: 7,
            coins: paid_coins(&spoon, true, &config(), &mut Rng::new(1)),
            given: Purse::default(),
            chapter: None,
        };
        assert_eq!(sale.change_due(), 18);
    }
}
