//! Counter and register: guess the wish, take payment, inspect coins,
//! hand out change – and decide what to do when a ghost pays twice.

use bevy::{ecs::system::SystemParam, prelude::*};

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::{Coin, Drawer, Purse, ghost_payment, smaller_payment};
use crate::fx::Cue;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::orders::{Order, OrderState};
use crate::rng::Rng;
use crate::shop::ShopLayout;
use crate::shop::Storeroom;
use crate::story_film::PlayStoryFilm;
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
                    .run_if(in_state(Phase::Night))
                    .run_if(crate::story_film::outside_film),
            );
    }
}

/// One coin the ghost put on the counter.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// The last handover was rejected; changing the tray clears the warning.
    pub recount: bool,
    /// (story, chapter) this sale completes, if it is a story ghost.
    pub chapter: Option<(usize, usize)>,
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
    progress: Res<Progress>,
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
    if ghost.first_customer
        && matches!(
            progress.journal.first_sale_checkpoint,
            Some(crate::guide::FirstSaleCheckpoint::Change { .. })
        )
    {
        actions.write(Action::Offer(ghost.wish.clone()));
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

#[derive(SystemParam)]
pub(crate) struct StoryEndings<'w> {
    guest: MessageWriter<'w, crate::ghosts::Finale>,
    film: MessageWriter<'w, PlayStoryFilm>,
}

/// A story ghost got what it came for: its thanks, its keepsake, and the
/// next chapter – or, after the last one, its ending (and a finale guest).
#[allow(clippy::too_many_arguments)]
fn complete_chapter(
    i: usize,
    chapter: usize,
    ghost: &mut Ghost,
    data: &GameData,
    lang: &Lang,
    progress: &mut Progress,
    storeroom: &mut Storeroom,
    report: &mut NightReport,
    log: &mut GameLog,
    cues: &mut MessageWriter<Cue>,
    endings: &mut StoryEndings,
) {
    let story = &data.stories[i];
    if progress.chapter(i) != chapter || chapter >= story.chapters.len() {
        ghost.state = GhostState::Leaving;
        return;
    }
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
    if progress.chapter(i) == story.chapters.len() {
        endings.film.write(PlayStoryFilm(story.ghost.clone()));
        log.push(lang.t(&format!("story.{}.ending", story.ghost)));
        report.redeemed.push(story.ghost.clone());
        cues.write(Cue::Ascend(ghost.pos));
        ghost.state = GhostState::Ascending;
        ghost.timer = 0.0;
        if let Some(guest) = &story.finale_guest {
            endings.guest.write(crate::ghosts::Finale(guest.clone()));
        }
    } else {
        ghost.state = GhostState::Leaving;
    }
}

/// Is the story ghost at the counter asking for a coin (its fare)?
pub fn asks_fare(ghost: &Ghost, data: &GameData) -> bool {
    ghost
        .chapter
        .is_some_and(|(i, c)| data.stories[i].chapters[c].fare)
}

/// Can the ghost at the counter order its wish for tomorrow? Only a plain
/// customer whose wish is not in stock, while orders for tomorrow are left.
pub fn can_order(ghost: &Ghost, stock: &Stock, progress: &Progress, max: usize) -> bool {
    let tomorrow = progress
        .orders
        .iter()
        .filter(|o| o.night == progress.night + 1)
        .count();
    ghost.chapter.is_none()
        && ghost.order.is_none()
        && stock.count(&ghost.wish) == 0
        && tomorrow < max
}

fn serve_front_ghost(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
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
    mut storeroom: ResMut<Storeroom>,
    mut endings: StoryEndings,
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
                if *product != ghost.wish {
                    if stock.count(product) == 0 {
                        continue;
                    }
                    if ghost.first_customer && progress.guiding_first_sale() {
                        log.push(lang.t("guide_retry"));
                        continue;
                    }
                    log.push(lang.f("log.wrong_item", &[("who", &lang.ghost_the(&def.id))]));
                    progress.set_order_state(ghost.order, OrderState::Expired);
                    report.wrong_item += 1;
                    ghost.state = GhostState::Leaving;
                    queue.remove(front);
                    progress.reputation -= 1;
                    report.unhappy += 1;
                    cues.write(Cue::Disappointed);
                    continue;
                }
                // Once payment is accepted the ware belongs to this ghost. The
                // apprentice, a thief or a mishap can still take other stock.
                if !parcel && !stock.take_one(product) {
                    continue;
                }
                let base = data.product(product).map_or(1, |p| p.price);
                let mut factor = def.pay_multiplier * data.price_factor(progress.calendar(&data));
                if ghost.order.is_some() {
                    factor *= 1.0 + data.config.orders.price_bonus;
                }
                let price = ((base as f32 * factor).round() as u32).max(1);
                let guided = ghost.first_customer && progress.guiding_first_sale();
                let purse = if guided {
                    crate::guide::payment(price, &def.pays_with)
                } else {
                    ghost_payment(price, &def.pays_with, &mut rng)
                };
                let counterfeit = !guided && rng.chance(def.quirks.counterfeit_chance);
                let (coins, given, recount) = if guided
                    && let Some(crate::guide::FirstSaleCheckpoint::Change {
                        coins,
                        given,
                        recount,
                    }) = progress.journal.first_sale_checkpoint.take()
                {
                    // Loaded from the night-start drawer: replay any doubts about genuine coins once.
                    progress.reputation -= coins.iter().filter(|c| c.checked).count() as i32;
                    (coins, given, recount)
                } else {
                    (
                        paid_coins(&purse, counterfeit, &lang, &mut rng),
                        Purse::default(),
                        false,
                    )
                };
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
                progress.set_order_state(ghost.order, OrderState::AtRegister);
                register.sale = Some(Sale {
                    ghost: front,
                    product: product.clone(),
                    price,
                    coins,
                    given,
                    recount,
                    chapter: ghost.chapter,
                    from_storeroom: ghost.carrying.is_none()
                        && !layout.displayed_products().contains(product),
                });
            }
            (Action::Decline, _) if !(ghost.first_customer && progress.guiding_first_sale()) => {
                report.declined += 1;
                if ghost.order.is_some() {
                    progress.set_order_state(ghost.order, OrderState::Expired);
                    // It came back for its order, and it is still missing.
                    progress.reputation -= data.config.orders.missed_penalty;
                    report.unhappy += 1;
                    cues.write(Cue::Disappointed);
                    log.push(lang.f(
                        "log.order_missed",
                        &[
                            ("who", &lang.ghost_the(&def.id)),
                            ("item", &lang.product(&ghost.wish)),
                        ],
                    ));
                } else {
                    log.push(lang.f("log.declined", &[("who", &lang.ghost_the(&def.id))]));
                }
                ghost.state = GhostState::Leaving;
                queue.remove(front);
            }
            // The ferryman's fare: the most precious coin in the drawer.
            (Action::GiveFare, _) if asks_fare(&ghost, &data) => {
                let Some(coin) = Coin::DESCENDING.into_iter().find(|c| drawer.get(*c) > 0) else {
                    continue;
                };
                let mut one = Purse::default();
                one.add(coin, 1);
                drawer.remove(&one);
                cues.write(Cue::ChangeGiven);
                log.push(lang.f(
                    "log.fare_given",
                    &[
                        ("who", &lang.ghost_the(&def.id)),
                        ("coin", &coin.name(&lang)),
                    ],
                ));
                let (i, chapter) = ghost.chapter.expect("checked by asks_fare");
                complete_chapter(
                    i,
                    chapter,
                    &mut ghost,
                    &data,
                    &lang,
                    &mut progress,
                    &mut storeroom,
                    &mut report,
                    &mut log,
                    &mut cues,
                    &mut endings,
                );
                queue.remove(front);
            }
            // Not in stock: the ghost orders it and comes back tomorrow.
            (Action::OrderForTomorrow, _)
                if can_order(&ghost, &stock, &progress, data.config.orders.max_per_night) =>
            {
                let night = progress.night + 1;
                let regular_name = ghost
                    .regular
                    .and_then(|i| progress.journal.regulars.get(i))
                    .map(|r| r.name);
                let order = Order::new(&def.id, &ghost.wish, night, regular_name);
                log.push(lang.f(
                    "log.ordered",
                    &[
                        ("who", &order.customer(&lang)),
                        ("item", &lang.product(&ghost.wish)),
                    ],
                ));
                progress.orders.push(order);
                ghost.state = GhostState::Leaving;
                queue.remove(front);
            }
            _ => {}
        }
    }
}

/// Inspect coins, count out change and hand it over.
pub(crate) fn make_change(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut register: ResMut<Register>,
    mut drawer: ResMut<Drawer>,
    mut queue: ResMut<Queue>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut storeroom: ResMut<Storeroom>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut ghosts: Query<&mut Ghost>,
    mut endings: StoryEndings,
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
                cues.write(Cue::CoinHandled(paid.coin));
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
                    sale.recount = false;
                    cues.write(Cue::CoinHandled(*coin));
                }
            }
            Action::RemoveChange(coin) => {
                let mut one = Purse::default();
                one.add(*coin, 1);
                if sale.given.remove(&one) {
                    sale.recount = false;
                    cues.write(Cue::CoinHandled(*coin));
                }
            }
            Action::ResetChange => {
                sale.given = Purse::default();
                sale.recount = false;
            }
            // The drawer cannot give back the change exactly: the ghost takes
            // its coins back and pays with smaller ones.
            Action::NoChange if !drawer.can_give(sale.change_due()) => {
                let Ok(ghost) = ghosts.get(sale.ghost) else {
                    continue;
                };
                let def = &data.ghosts[ghost.def];
                drawer.remove(&sale.paid());
                let purse = smaller_payment(sale.price, &def.pays_with, &drawer.0);
                drawer.merge(&purse);
                sale.coins = paid_coins(&purse, false, &lang, &mut rng);
                sale.given = Purse::default();
                sale.recount = false;
                cues.write(Cue::Paid);
                log.push(lang.f(
                    "log.smaller_coins",
                    &[
                        ("who", &lang.ghost_the(&def.id)),
                        ("coins", &purse.describe(&lang)),
                    ],
                ));
            }
            Action::GiveChange | Action::GiveChangeAnyway => {
                let guided = ghosts
                    .get(sale.ghost)
                    .is_ok_and(|g| g.first_customer && progress.guiding_first_sale());
                if matches!(action, Action::GiveChangeAnyway) {
                    if guided || !sale.recount {
                        continue;
                    }
                } else if sale.given.total() != sale.change_due() {
                    sale.recount = true;
                    continue;
                }
                let sale = register.sale.take().expect("checked above");
                drawer.remove(&sale.given);
                let given = sale.given.total();
                report.record_sale(&sale.product, sale.price, sale.paid().total(), given);
                if let Some(index) = ghosts.get(sale.ghost).ok().and_then(|g| g.order) {
                    progress.set_order_state(Some(index), OrderState::Collected);
                    report.orders_fulfilled += 1;
                }
                report
                    .counterfeits_taken
                    .extend(sale.coins.iter().filter(|c| c.fake).map(|c| c.coin));

                let due = sale.change_due();
                report.change_kept += due as i64 - given as i64;
                let Ok(mut ghost) = ghosts.get_mut(sale.ghost) else {
                    queue.remove(sale.ghost);
                    continue;
                };
                let def = &data.ghosts[ghost.def];
                let name = lang.ghost_the(&def.id);
                let correct = given == due;
                let guided = ghost.first_customer && progress.guiding_first_sale();
                if guided {
                    progress.journal.first_sale = crate::guide::FirstSale::Complete;
                }
                progress
                    .journal
                    .served(&def.id, &sale.product, ghost.regular);
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
                    // A shop the spirits speak well of gets a tip now and then.
                    let perks = data.config.reputation_perks;
                    // Regulars tip more often the more often they come.
                    let regular = ghost.regular.and_then(|i| progress.journal.regulars.get(i));
                    let bonus = crate::regulars::tip_bonus(&data, regular);
                    if rng.chance(perks.tip_chance(progress.reputation) + bonus) {
                        let coin =
                            if progress.reputation >= perks.candle_tip_from && rng.chance(0.5) {
                                Coin::Candle
                            } else {
                                Coin::Button
                            };
                        drawer.0.add(coin, 1);
                        report.tips += coin.value();
                        report.tips_given += 1;
                        cues.write(Cue::Tip(ghost.pos, coin));
                        log.push(lang.f("log.tip", &[("who", &name), ("coin", &coin.name(&lang))]));
                    }
                }

                if let Some((i, chapter)) = sale.chapter {
                    // The story ghost got what it needed, whatever the change.
                    complete_chapter(
                        i,
                        chapter,
                        &mut ghost,
                        &data,
                        &lang,
                        &mut progress,
                        &mut storeroom,
                        &mut report,
                        &mut log,
                        &mut cues,
                        &mut endings,
                    );
                } else if !correct {
                    ghost.state = GhostState::Leaving;
                } else if !guided && rng.chance(def.quirks.double_pay_chance) {
                    log.push(lang.f("log.double_pay", &[("who", &name), ("amount", &sale.price)]));
                    register.question = Some(DoublePay {
                        ghost: sale.ghost,
                        amount: sale.price,
                    });
                } else {
                    // Served well, it may come back as a regular.
                    if ghost.regular.is_none()
                        && let Some(regular) = crate::regulars::maybe_new(
                            &data,
                            &mut progress,
                            def,
                            &sale.product,
                            &lang,
                            &mut rng,
                        )
                    {
                        ghost.regular = Some(regular);
                        let regular_name = progress.journal.regulars[regular].name(&lang);
                        log.push(lang.f(
                            "log.regular_new",
                            &[("who", &name), ("name", &regular_name)],
                        ));
                    }
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
            recount: false,
            chapter: None,
            from_storeroom: false,
        };
        assert_eq!(sale.change_due(), 18);
    }
}
