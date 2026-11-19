//! Counter and register: guess the wish, take payment, inspect coins,
//! hand out change – and decide what to do when a ghost pays twice.

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::{Coin, Drawer, Purse, ghost_payment};
use crate::fx::Cue;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::rng::Rng;
use crate::shop::ShopLayout;
use crate::shop::Storeroom;
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
                (
                    bring_carried_ware,
                    serve_front_ghost,
                    make_change,
                    answer_double_pay,
                )
                    .chain()
                    .run_if(in_state(Phase::Night)),
            );
    }
}

/// One coin the ghost put on the counter.
#[derive(Clone, Debug)]
pub struct PaidCoin {
    pub coin: Coin,
    /// Catalog key of how the coin looks or feels – counterfeits give
    /// themselves away here.
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
    /// (story, chapter) this sale completes, if it is a story ghost.
    pub chapter: Option<(usize, usize)>,
    /// The ferryman's parcel for a story ghost: it does not come out of stock.
    pub parcel: bool,
    /// Fetched from the storeroom, not found on the shelves: correct change
    /// earns no reputation then.
    pub from_storeroom: bool,
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
pub fn paid_coins(purse: &Purse, counterfeit: bool, lang: &Lang, rng: &mut Rng) -> Vec<PaidCoin> {
    let genuine = lang.count("tell.genuine.");
    let fakes = lang.count("tell.fake.");
    let mut coins = Vec::new();
    for coin in Coin::DESCENDING {
        for _ in 0..purse.get(coin) {
            coins.push(PaidCoin {
                coin,
                tell: format!("tell.genuine.{}", rng.index(genuine) + 1),
                fake: false,
                checked: false,
            });
        }
    }
    if counterfeit && !coins.is_empty() {
        let i = rng.index(coins.len());
        coins[i].fake = true;
        coins[i].tell = format!("tell.fake.{}", rng.index(fakes) + 1);
    }
    coins
}

/// The player hands an item to the ghost at the front – or declines.
/// A ghost at the front that brings a ware from the shelf puts it on the
/// counter: the sale starts by itself. If the last one was sold meanwhile,
/// it whispers its wish like any other ghost.
fn bring_carried_ware(
    register: Res<Register>,
    queue: Res<Queue>,
    stock: Res<Stock>,
    mut ghosts: Query<&mut Ghost>,
    mut actions: MessageWriter<Action>,
) {
    if register.is_busy() {
        return;
    }
    let Some(mut ghost) = queue.front().and_then(|e| ghosts.get_mut(e).ok()) else {
        return;
    };
    if ghost.state != GhostState::Waiting {
        return;
    }
    if let Some(ware) = ghost.carrying.clone() {
        if stock.count(&ware) > 0 {
            actions.write(Action::Offer(ware));
        } else {
            ghost.carrying = None;
        }
    }
}

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
    mut cues: MessageWriter<Cue>,
    mut ghosts: Query<&mut Ghost>,
    lang: Res<Lang>,
    layout: Res<ShopLayout>,
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

        // The ferryman's parcel holds exactly what this story ghost needs.
        let parcel = *action == Action::HandParcel;
        let parcel_for = ghost
            .chapter
            .map(|(story, _)| story)
            .filter(|s| progress.parcels.contains(s));
        let offer = match action {
            Action::Offer(product) => Some(product.clone()),
            Action::HandParcel if parcel_for.is_some() => Some(ghost.wish.clone()),
            _ => None,
        };
        match (action, offer) {
            (Action::Offer(_) | Action::HandParcel, Some(product)) => {
                let product = &product;
                if !parcel && stock.count(product) == 0 {
                    continue;
                }
                if *product != ghost.wish {
                    log.push(lang.f("log.wrong_item", &[("who", &lang.ghost_the(&def.id))]));
                    report.wrong_item += 1;
                    ghost.state = GhostState::Leaving;
                    queue.remove(front);
                    progress.reputation -= 1;
                    report.unhappy += 1;
                    cues.write(Cue::Disappointed);
                    continue;
                }
                let base = data.product(product).map_or(1, |p| p.price);
                let factor = def.pay_multiplier * data.price_factor(progress.night);
                let price = ((base as f32 * factor).round() as u32).max(1);
                let purse = ghost_payment(price, &def.pays_with, &mut rng);
                let counterfeit = rng.chance(def.quirks.counterfeit_chance);
                let coins = paid_coins(&purse, counterfeit, &lang, &mut rng);
                // Payment goes straight into the drawer – change is taken from there too.
                drawer.merge(&purse);
                cues.write(Cue::Paid);
                ghost.state = GhostState::AtRegister;
                log.push(lang.f(
                    "log.paid",
                    &[
                        ("who", &lang.ghost_the(&def.id)),
                        ("coins", &purse.describe(&lang)),
                    ],
                ));
                register.sale = Some(Sale {
                    ghost: front,
                    product: product.clone(),
                    price,
                    coins,
                    given: Purse::default(),
                    chapter: ghost.chapter,
                    parcel,
                    from_storeroom: ghost.carrying.is_none()
                        && !layout.displayed_products().contains(product),
                });
            }
            (Action::Decline, _) => {
                report.declined += 1;
                log.push(lang.f("log.declined", &[("who", &lang.ghost_the(&def.id))]));
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
    mut storeroom: ResMut<Storeroom>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut ghosts: Query<&mut Ghost>,
    mut finale: MessageWriter<crate::ghosts::Finale>,
    lang: Res<Lang>,
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
                    .map(|g| lang.ghost_the(&data.ghosts[g.def].id))
                    .unwrap_or_default();
                if paid.fake {
                    // The ghost swaps it for a genuine one of the same kind,
                    // so the drawer total stays as it is.
                    paid.fake = false;
                    paid.tell = "register.swapped".into();
                    report.counterfeits_caught += 1;
                    log.push(lang.f(
                        "log.fake_caught",
                        &[("coin", &paid.coin.name(&lang)), ("who", &name)],
                    ));
                } else {
                    progress.reputation -= 1;
                    log.push(lang.f(
                        "log.genuine_doubted",
                        &[("coin", &paid.coin.name(&lang)), ("who", &name)],
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
                if !sale.parcel {
                    stock.take_one(&sale.product);
                }
                report.sales += 1;
                report.revenue += sale.price;
                report.sold.push(sale.product.clone());
                report
                    .counterfeits_taken
                    .extend(sale.coins.iter().filter(|c| c.fake).map(|c| c.coin));

                let due = sale.change_due();
                let given = sale.given.total();
                report.change_kept += due as i64 - given as i64;
                let Ok(mut ghost) = ghosts.get_mut(sale.ghost) else {
                    queue.remove(sale.ghost);
                    continue;
                };
                let def = &data.ghosts[ghost.def];
                let name = lang.ghost_the(&def.id);
                let correct = given == due;
                if given > 0 {
                    cues.write(Cue::ChangeGiven);
                }

                if given < due {
                    report.change_wrong += 1;
                    progress.reputation -= 2;
                    cues.write(Cue::Disappointed);
                    log.push(lang.f(
                        "log.too_little",
                        &[("given", &given), ("due", &due), ("who", &name)],
                    ));
                } else if given > due {
                    report.change_wrong += 1;
                    log.push(lang.f(
                        "log.too_much",
                        &[("given", &given), ("due", &due), ("who", &name)],
                    ));
                } else {
                    report.change_correct += 1;
                    // Found on the shelves: the ghost leaves happier.
                    if !sale.from_storeroom {
                        progress.reputation += 1;
                    }
                }

                if let Some((i, chapter)) = sale.chapter {
                    // The story ghost got what it needed, whatever the change.
                    let story = &data.stories[i];
                    let c = &story.chapters[chapter];
                    let thanks = format!("story.{}.{}.thanks", story.ghost, chapter + 1);
                    log.push(lang.f(
                        "log.story_says",
                        &[
                            ("name", &lang.ghost(&story.ghost)),
                            ("text", &lang.t(&thanks)),
                        ],
                    ));
                    report.story_lines.push((story.ghost.clone(), thanks));
                    report.story_done.push(i);
                    if let Some(k) = &c.keepsake {
                        storeroom.give(k);
                        report.keepsakes.push(k.clone());
                    }
                    progress.finish_chapter(i);
                    if progress.chapter(i) >= story.chapters.len() {
                        log.push(lang.t(&format!("story.{}.ending", story.ghost)));
                        report.redeemed.push(story.ghost.clone());
                        cues.write(Cue::Ascend(ghost.pos));
                        ghost.state = GhostState::Ascending;
                        ghost.timer = 0.0;
                        if story.finale {
                            finale.write(crate::ghosts::Finale);
                        }
                    } else {
                        ghost.state = GhostState::Leaving;
                    }
                } else if !correct {
                    ghost.state = GhostState::Leaving;
                } else if rng.chance(def.quirks.double_pay_chance) {
                    log.push(lang.f("log.double_pay", &[("who", &name), ("amount", &sale.price)]));
                    register.question = Some(DoublePay {
                        ghost: sale.ghost,
                        amount: sale.price,
                    });
                } else {
                    cues.write(Cue::Ascend(ghost.pos));
                    ghost.state = GhostState::Ascending;
                    ghost.timer = 0.0;
                    report.redeemed.push(def.id.clone());
                    log.push(lang.f(
                        "log.ascend",
                        &[("who", &name), ("item", &lang.product(&sale.product))],
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
    mut cues: MessageWriter<Cue>,
    mut ghosts: Query<&mut Ghost>,
    lang: Res<Lang>,
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
        let name = lang.ghost_the(&def.id);
        if keep {
            drawer.merge(&Purse::greedy(question.amount));
            report.second_payments_kept += question.amount;
            cues.write(Cue::Paid);
            progress.reputation -= 1;
            report.dishonest += 1;
            ghost.state = GhostState::Leaving;
            log.push(lang.f("log.kept_payment", &[("who", &name)]));
        } else {
            progress.reputation += 2;
            report.honest += 1;
            cues.write(Cue::Ascend(ghost.pos));
            report.redeemed.push(def.id.clone());
            ghost.state = GhostState::Ascending;
            ghost.timer = 0.0;
            log.push(lang.f("log.refused_payment", &[("who", &name)]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang() -> Lang {
        Lang::load("en")
    }

    #[test]
    fn paid_coins_keep_the_payment_and_fake_at_most_one() {
        let lang = lang();
        let mut rng = Rng::new(7);
        let purse = Purse {
            buttons: 2,
            candles: 1,
            spoons: 1,
        };
        for counterfeit in [false, true] {
            let coins = paid_coins(&purse, counterfeit, &lang, &mut rng);
            assert_eq!(coins.len(), 4);
            let mut total = Purse::default();
            for c in &coins {
                total.add(c.coin, 1);
            }
            assert_eq!(total, purse);
            let fakes: Vec<_> = coins.iter().filter(|c| c.fake).collect();
            assert_eq!(fakes.len(), usize::from(counterfeit));
            for f in fakes {
                assert!(f.tell.starts_with("tell.fake."));
                assert_ne!(lang.t(&f.tell), f.tell, "tell has a text");
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
            coins: paid_coins(&spoon, true, &lang(), &mut Rng::new(1)),
            given: Purse::default(),
            chapter: None,
            parcel: false,
            from_storeroom: false,
        };
        assert_eq!(sale.change_due(), 18);
    }
}
