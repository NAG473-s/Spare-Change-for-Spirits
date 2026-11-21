//! Daily cycle: evening (preparation) → night (open) → morning (accounts).

use bevy::prelude::*;

use crate::data::GameData;
use crate::economy::{Coin, Drawer, Purse};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::orders::Order;
use crate::register::Register;
use crate::ui::{Action, GameLog};

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    #[default]
    Menu,
    /// The village market, before the evening (from the second day on).
    Day,
    Evening,
    Night,
    Morning,
    DemoEnd,
}

impl Phase {
    /// Catalog key of the phase name.
    pub fn key(self) -> &'static str {
        match self {
            Phase::Menu => "phase.menu",
            Phase::Day => "phase.day",
            Phase::Evening => "phase.evening",
            Phase::Night => "phase.night",
            Phase::Morning => "phase.morning",
            // The full game reaches the end of the season instead.
            Phase::DemoEnd if cfg!(feature = "demo") => "phase.demo_end",
            Phase::DemoEnd => "end.title_full",
        }
    }
}

pub struct DayCyclePlugin;

impl Plugin for DayCyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Phase>()
            .init_resource::<NightClock>()
            .init_resource::<NightReport>()
            .add_systems(
                OnEnter(Phase::Night),
                (start_night, deliver_parcels).chain(),
            )
            .add_systems(OnEnter(Phase::Morning), (start_morning, count_story_tries))
            .add_systems(
                Update,
                (
                    (tick_clock, close_at_dawn)
                        .chain()
                        .run_if(in_state(Phase::Night))
                        .run_if(crate::story_film::outside_film),
                    next_evening.run_if(in_state(Phase::Morning)),
                    end_day.run_if(in_state(Phase::Day)),
                    keep_playing.run_if(in_state(Phase::DemoEnd)),
                ),
            );
    }
}

/// Running game state across all nights.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Progress {
    pub night: u32,
    pub reputation: i32,
    /// Next chapter of each story in `story.ron` (same order); equal to its
    /// chapter count once that story is finished.
    pub story: Vec<usize>,
    /// Nights the current chapter of each story was tried without success.
    pub story_tries: Vec<u32>,
    /// Stories whose ghost has a parcel from the ferryman waiting.
    pub parcels: Vec<usize>,
    /// Wares ghosts ordered for a coming night.
    pub orders: Vec<Order>,
    /// Broken things found in the junk, waiting at the workbench (product ids).
    pub broken: Vec<String>,
    /// The last night the junk was searched / something was repaired.
    pub rummaged_on: u32,
    pub repaired_on: u32,
    /// Counters over the whole game, for the achievements (`Stat::key`).
    pub tally: std::collections::BTreeMap<String, u32>,
    /// Achievements unlocked in this game (ids from achievements.ron).
    pub achievements: Vec<String>,
    /// The season the shop is in (1, 2, …): each tells its own story over
    /// `season_length` nights and lasts until its finale is done.
    pub season: u32,
    /// The night the current season began.
    pub season_start: u32,
    /// The ghost book, the regulars and the shop window (`book.rs`).
    pub journal: crate::book::Journal,
}

/// From this many failed nights on, a story ghost names the ware it needs.
pub const PLAIN_WISH_AFTER: u32 = 2;

impl Progress {
    pub fn new(reputation: i32) -> Self {
        Self {
            night: 1,
            reputation,
            story: Vec::new(),
            story_tries: Vec::new(),
            parcels: Vec::new(),
            orders: Vec::new(),
            broken: Vec::new(),
            rummaged_on: 0,
            repaired_on: 0,
            tally: Default::default(),
            achievements: Vec::new(),
            season: 1,
            season_start: 1,
            journal: Default::default(),
        }
    }

    /// The night in the story's calendar: what events, unlocks and story
    /// ghosts go by. Season 2 starts at calendar night `season_length + 1`
    /// whenever season 1 actually ended; while a season waits for its finale
    /// past its last night, its last calendar night repeats.
    pub fn calendar(&self, data: &GameData) -> u32 {
        let len = data.config.season_length;
        let into = (self.night + 1)
            .saturating_sub(self.season_start)
            .clamp(1, len);
        (self.season - 1) * len + into
    }

    /// The night the current season is meant to end (later if its finale is
    /// still open).
    pub fn season_end(&self, data: &GameData) -> u32 {
        self.season_start + data.config.season_length - 1
    }

    /// Does the current season end with a finale story?
    pub fn season_has_finale(&self, data: &GameData) -> bool {
        data.stories
            .iter()
            .any(|s| s.finale && data.season_of(s.start_night) == self.season)
    }

    /// The next season begins the night after this one.
    pub fn next_season(&mut self) {
        self.night += 1;
        self.season += 1;
        self.season_start = self.night;
    }

    /// The next chapter of story `i`.
    pub fn chapter(&self, i: usize) -> usize {
        self.story.get(i).copied().unwrap_or(0)
    }

    /// Nights the current chapter of story `i` has failed.
    pub fn tries(&self, i: usize) -> u32 {
        self.story_tries.get(i).copied().unwrap_or(0)
    }

    /// Does the ghost of story `i` come tonight?
    pub fn story_due(&self, data: &GameData, i: usize) -> bool {
        data.story_due(i, self.calendar(data), self.chapter(i))
    }

    /// Does the ghost of story `i` say plainly what it needs?
    pub fn asks_plainly(&self, i: usize) -> bool {
        self.tries(i) >= PLAIN_WISH_AFTER
    }

    /// Story `i` moves on to its next chapter.
    pub fn finish_chapter(&mut self, i: usize) {
        if self.story.len() <= i {
            self.story.resize(i + 1, 0);
        }
        if self.story_tries.len() <= i {
            self.story_tries.resize(i + 1, 0);
        }
        self.story[i] += 1;
        self.story_tries[i] = 0;
        self.parcels.retain(|p| *p != i);
        self.journal.set_aside.retain(|p| *p != i);
        // For logbook keepsakes: when each chapter was finished.
        self.journal
            .chapter_nights
            .entry(i)
            .or_default()
            .push(self.night);
    }

    /// A counter for the achievements.
    pub fn tally(&self, stat: crate::achievements::Stat) -> u32 {
        self.tally.get(stat.key()).copied().unwrap_or(0)
    }

    pub fn count(&mut self, stat: crate::achievements::Stat, n: u32) {
        if n > 0 {
            *self.tally.entry(stat.key().to_string()).or_default() += n;
        }
    }

    /// The current chapter of story `i` failed tonight.
    pub fn fail_chapter(&mut self, i: usize) {
        if self.story_tries.len() <= i {
            self.story_tries.resize(i + 1, 0);
        }
        self.story_tries[i] += 1;
    }

    /// Is the finale of the current season behind us? A season without a
    /// finale story never ends: the shop just stays open.
    pub fn finale_done(&self, data: &GameData) -> bool {
        self.season_has_finale(data)
            && data
                .stories
                .iter()
                .enumerate()
                .filter(|(_, s)| s.finale && data.season_of(s.start_night) == self.season)
                .all(|(i, s)| self.chapter(i) >= s.chapters.len())
    }

    /// Does the season end with this night? The demo stops at its last
    /// night; the full game's seasons last `season_length` nights or until
    /// their finale is done, whichever is later.
    pub fn season_over(&self, data: &GameData) -> bool {
        if cfg!(feature = "demo") {
            return self.night >= data.config.final_night();
        }
        self.night >= self.season_end(data) && self.finale_done(data)
    }
}

#[derive(Resource, Default)]
pub struct NightClock {
    pub elapsed: f32,
    pub length: f32,
}

impl NightClock {
    pub fn is_over(&self) -> bool {
        self.elapsed >= self.length
    }

    /// In-game clock: 00:00 to 05:00.
    pub fn time_string(&self) -> String {
        let minutes = (self.elapsed / self.length.max(1.0) * 300.0).min(300.0) as u32;
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    }
}

/// A settled sale. Amounts are in buttons, using the final payment after
/// any swap for smaller coins and the change actually handed over.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SaleReceipt {
    pub product: String,
    pub price: u32,
    pub paid: u32,
    pub given: u32,
}

/// Statistics of one night, shown in the morning.
#[derive(Resource, Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NightReport {
    pub sales: u32,
    pub revenue: u32,
    pub change_correct: u32,
    pub change_wrong: u32,
    pub unhappy: u32,
    pub fled: u32,
    pub redeemed: Vec<String>,
    pub faded: Vec<(String, u32)>,
    pub reputation_start: i32,
    /// Ids of ghosts that passed into the light tonight are in `redeemed`.
    /// Items the poltergeist knocked off the shelves (product ids).
    pub knocked: Vec<String>,
    /// Counterfeit coins spotted at the register.
    pub counterfeits_caught: u32,
    /// Counterfeit coins accepted without noticing.
    pub counterfeits_taken: Vec<Coin>,
    /// Counterfeit coins found in the drawer in the morning (removed from it).
    pub counterfeits_found: Purse,
    /// Double payments refused / kept.
    pub honest: u32,
    pub dishonest: u32,
    /// Stories (index in `story.ron`) whose ghost has come tonight.
    pub story_arrived: Vec<usize>,
    /// Stories whose chapter was finished tonight.
    pub story_done: Vec<usize>,
    /// (ghost id, catalog key) of what story ghosts said tonight when served.
    pub story_lines: Vec<(String, String)>,
    /// Keepsakes received tonight (decor ids).
    pub keepsakes: Vec<String>,
    /// Ghosts that came in (or tried to) tonight.
    pub visitors: u32,
    pub receipts: Vec<SaleReceipt>,
    /// Change due minus change given, summed over all sales: positive when
    /// too little change was handed out (the drawer keeps it).
    pub change_kept: i64,
    /// Value of second payments that were kept.
    pub second_payments_kept: u32,
    /// Tips left for exact change (in button units).
    pub tips: u32,
    /// Wares a shade stole from the shelves (product ids).
    pub stolen: Vec<String>,
    /// How many tips were left, orders fulfilled and mishaps fixed tonight.
    pub tips_given: u32,
    pub orders_fulfilled: u32,
    /// Indices of order slips whose customers have arrived this night.
    pub dispatched_orders: Vec<usize>,
    pub mishaps_fixed: u32,
    /// Customers the apprentice served (`apprentice.rs`).
    pub apprentice_served: u32,
    /// Ghosts turned away because their wish was not in stock.
    pub declined: u32,
    /// Ghosts offered the wrong item.
    pub wrong_item: u32,
}

impl NightReport {
    pub fn record_sale(&mut self, product: &str, price: u32, paid: u32, given: u32) {
        self.sales += 1;
        self.revenue += price;
        self.receipts.push(SaleReceipt {
            product: product.to_owned(),
            price,
            paid,
            given,
        });
    }
}

fn start_night(
    data: Res<GameData>,
    progress: Res<Progress>,
    market: Res<crate::market::MarketDay>,
    mut clock: ResMut<NightClock>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    lang: Res<Lang>,
) {
    *clock = NightClock {
        elapsed: 0.0,
        length: data.night_length(progress.calendar(&data))
            * market.night_length_factor(progress.night),
    };
    if let Some(settled) = progress
        .journal
        .first_sale_checkpoint
        .as_ref()
        .and_then(|c| c.settled_report())
    {
        *report = settled.clone();
        return;
    }
    *report = NightReport {
        reputation_start: progress.reputation,
        ..default()
    };
    log.push(lang.t("log.midnight"));
}

fn tick_clock(time: Res<Time>, progress: Res<Progress>, mut clock: ResMut<NightClock>) {
    if !progress.guiding_first_sale() {
        clock.elapsed += time.delta_secs();
    }
}

/// At dawn all ghosts leave. Once the shop is empty, morning begins.
fn close_at_dawn(
    clock: Res<NightClock>,
    mut progress: ResMut<Progress>,
    register: Res<Register>,
    mut queue: ResMut<Queue>,
    mut ghosts: Query<(Entity, &mut Ghost)>,
    mut next: ResMut<NextState<Phase>>,
) {
    if !clock.is_over() {
        return;
    }
    for (entity, mut ghost) in &mut ghosts {
        let busy = register.is_busy_with(entity);
        if !busy
            && matches!(
                ghost.state,
                GhostState::Entering
                    | GhostState::Browsing
                    | GhostState::ToCounter
                    | GhostState::Waiting
            )
        {
            progress.set_order_state(ghost.order, crate::orders::OrderState::Expired);
            ghost.state = GhostState::Leaving;
            queue.remove(entity);
        }
    }
    if ghosts.is_empty() {
        next.set(Phase::Morning);
    }
}

/// The ferryman makes sure no story gets stuck: a story ghost whose wish is
/// not in stock when the shop opens – or who already left empty-handed
/// once – gets a parcel holding exactly what it needs.
pub fn deliver_parcels(
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
    mut progress: ResMut<Progress>,
    mut log: ResMut<GameLog>,
    lang: Res<Lang>,
) {
    if progress
        .journal
        .first_sale_checkpoint
        .as_ref()
        .is_some_and(|c| c.settled_report().is_some())
    {
        return;
    }
    for (i, story) in data.stories.iter().enumerate() {
        if !progress.story_due(&data, i) || progress.parcels.contains(&i) {
            continue;
        }
        let chapter = &story.chapters[progress.chapter(i)];
        // A fare is paid from the drawer: no parcel for that.
        if chapter.fare {
            continue;
        }
        let wish = &chapter.wish;
        // The announced ware is in stock: one is set aside for the story
        // ghost, so the ordinary customers cannot take the last one before it
        // comes. It is handed over like a parcel.
        if progress.tries(i) == 0 && stock.take_one(wish) {
            progress.parcels.push(i);
            progress.journal.set_aside.push(i);
            let message = if progress.journal.has_seen(&story.ghost) {
                lang.f(
                    "log.set_aside",
                    &[
                        ("item", &lang.product(wish)),
                        ("name", &lang.ghost(&story.ghost)),
                    ],
                )
            } else {
                lang.f("discovery.set_aside", &[("item", &lang.product(wish))])
            };
            log.push(message);
            continue;
        }
        if stock.count(wish) == 0 || progress.tries(i) > 0 {
            progress.parcels.push(i);
            let message = if progress.journal.has_seen(&story.ghost) {
                lang.f("log.parcel", &[("name", &lang.ghost(&story.ghost))])
            } else {
                lang.t("discovery.parcel_waits")
            };
            log.push(message);
        }
    }
}

/// After a season's finale: on to the next season.
fn keep_playing(
    mut actions: MessageReader<Action>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<Phase>>,
) {
    if actions.read().any(|a| *a == Action::KeepPlaying) {
        progress.next_season();
        next.set(Phase::Day);
    }
}

/// A story ghost that came tonight and did not get its wish tries again
/// tomorrow – a little more plainly each time.
pub fn count_story_tries(mut progress: ResMut<Progress>, report: Res<NightReport>) {
    // Tonight's orders are settled, whatever happened.
    progress.archive_due_orders();
    for &i in &report.story_arrived {
        if !report.story_done.contains(&i) {
            progress.fail_chapter(i);
        }
    }
}

pub fn start_morning(
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
    mut drawer: ResMut<Drawer>,
    mut report: ResMut<NightReport>,
    upgrades: Res<crate::upgrades::Upgrades>,
) {
    report.faded = stock.age_and_fade(&data, upgrades.fade_bonus(&data));
    report.counterfeits_found = remove_counterfeits(&mut drawer.0, &report.counterfeits_taken);
}

/// Counterfeit coins that are still in the drawer turn to dust at dawn.
/// Those already handed out as change are gone anyway.
pub fn remove_counterfeits(drawer: &mut Purse, taken: &[Coin]) -> Purse {
    let mut found = Purse::default();
    for coin in taken {
        let mut one = Purse::default();
        one.add(*coin, 1);
        if drawer.remove(&one) {
            found.add(*coin, 1);
        }
    }
    found
}

fn next_evening(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<Phase>>,
) {
    if !actions.read().any(|a| *a == Action::NextEvening) {
        return;
    }
    if progress.season_over(&data) {
        next.set(Phase::DemoEnd);
    } else {
        progress.night += 1;
        next.set(Phase::Day);
    }
}

/// Leaving the market: back to the shop for the evening.
fn end_day(mut actions: MessageReader<Action>, mut next: ResMut<NextState<Phase>>) {
    if actions.read().any(|a| *a == Action::EndDay) {
        next.set(Phase::Evening);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_ledger_and_market_buttons_reach_the_second_evening() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_state(Phase::Morning)
            .add_message::<Action>()
            .insert_resource(GameData::load())
            .insert_resource(Progress::new(0))
            .add_systems(
                Update,
                (
                    next_evening.run_if(in_state(Phase::Morning)),
                    end_day.run_if(in_state(Phase::Day)),
                ),
            );
        app.update();
        for (button, destination) in [
            (Action::NextEvening, Phase::Day),
            (Action::EndDay, Phase::Evening),
        ] {
            app.world_mut().write_message(button);
            app.update();
            app.update();
            assert_eq!(*app.world().resource::<State<Phase>>().get(), destination);
            assert_eq!(app.world().resource::<Progress>().night, 2);
        }
    }

    #[test]
    fn counterfeits_are_removed_only_if_still_in_drawer() {
        let mut drawer = Purse {
            buttons: 3,
            candles: 0,
            spoons: 1,
        };
        let found = remove_counterfeits(&mut drawer, &[Coin::Spoon, Coin::Candle]);
        assert_eq!(
            found,
            Purse {
                buttons: 0,
                candles: 0,
                spoons: 1
            }
        );
        assert_eq!(
            drawer,
            Purse {
                buttons: 3,
                candles: 0,
                spoons: 0
            }
        );
    }
    /// Plays the stories with one chapter per night, the given nights failing.
    #[cfg(not(feature = "demo"))]
    fn play(data: &GameData, failures: &[u32]) -> Progress {
        play_from(Progress::new(0), data, failures)
    }

    #[cfg(not(feature = "demo"))]
    fn play_from(mut progress: Progress, data: &GameData, failures: &[u32]) -> Progress {
        loop {
            for i in 0..data.stories.len() {
                if progress.story_due(data, i) {
                    if failures.contains(&progress.night) {
                        progress.fail_chapter(i);
                    } else {
                        progress.finish_chapter(i);
                    }
                }
            }
            if progress.season_over(data) {
                return progress;
            }
            progress.night += 1;
            assert!(progress.night < 100, "the season never ends");
        }
    }

    // The demo stops after its last night instead.
    #[cfg(not(feature = "demo"))]
    #[test]
    fn a_season_lasts_until_its_finale() {
        let data = GameData::load();
        let len = data.config.season_length;
        let smooth = play(&data, &[]);
        assert_eq!(smooth.night, len, "every chapter on time");
        // Two failed nights push the finale back by two nights; meanwhile
        // the season's last calendar night repeats.
        let late = play(&data, &[7, 8]);
        assert_eq!(late.night, len + 2);
        assert_eq!(late.calendar(&data), len);
        assert!(late.finale_done(&data));
        // The next season starts its own calendar the night after.
        let mut next = late.clone();
        next.next_season();
        assert_eq!((next.season, next.night), (2, len + 3));
        assert_eq!(next.calendar(&data), len + 1);
        assert!(!next.season_over(&data));
        // Season 2 tells its own story and ends with its own finale.
        let mut second = smooth.clone();
        second.next_season();
        let second = play_from(second, &data, &[]);
        assert_eq!((second.season, second.night), (2, 2 * len));
        assert!(crate::winter::snowing(&data, &second));
        assert_eq!(crate::winter::clock_level(&data, &second), 1.0);
        // Season 3: the thaw ends the snow, and the ferryman's fare ends the season.
        let mut third = second.clone();
        third.next_season();
        let third = play_from(third, &data, &[]);
        assert_eq!((third.season, third.night), (3, 3 * len));
        assert!(!crate::winter::snowing(&data, &third));
        assert_eq!(crate::winter::blossom_level(&data, &third), 1.0);
    }

    #[test]
    fn an_announced_ware_in_stock_is_set_aside_for_the_story_ghost() {
        use bevy::ecs::system::RunSystemOnce;
        let data = GameData::load();
        let edwin = data
            .stories
            .iter()
            .position(|s| s.ghost == "edwin")
            .unwrap();
        let wish = data.stories[edwin].chapters[0].wish.clone();
        let mut world = World::new();
        let mut stock = Stock::default();
        stock.add(&wish, 1);
        let mut progress = Progress::new(0);
        progress.night = data.stories[edwin].start_night;
        world.insert_resource(stock);
        world.insert_resource(progress);
        world.insert_resource(data);
        world.insert_resource(Lang::load("en"));
        world.init_resource::<GameLog>();
        world.run_system_once(deliver_parcels).unwrap();
        // The last one is kept back for Edwin, not sold to anyone else.
        assert_eq!(world.resource::<Stock>().count(&wish), 0);
        let progress = world.resource::<Progress>();
        assert!(progress.parcels.contains(&edwin));
        assert!(progress.journal.set_aside.contains(&edwin));
        // Once his chapter is done, nothing stays set aside.
        let mut progress = world.resource_mut::<Progress>();
        progress.finish_chapter(edwin);
        assert!(progress.parcels.is_empty() && progress.journal.set_aside.is_empty());
    }

    #[test]
    fn a_failed_chapter_is_named_plainly_and_cleared_when_done() {
        let mut progress = Progress::new(0);
        assert!(!progress.asks_plainly(0));
        for _ in 0..PLAIN_WISH_AFTER {
            progress.fail_chapter(0);
        }
        assert!(progress.asks_plainly(0));
        progress.parcels.push(0);
        progress.finish_chapter(0);
        assert_eq!(progress.chapter(0), 1);
        assert!(!progress.asks_plainly(0));
        assert!(progress.parcels.is_empty(), "the parcel is used up");
    }

    #[test]
    fn the_lighthouse_brightens_with_the_story() {
        let data = GameData::load();
        let (i, story) = data
            .stories
            .iter()
            .enumerate()
            .find(|(_, s)| s.lighthouse)
            .expect("a story lights the lighthouse");
        let mut progress = Progress::new(0);
        assert_eq!(crate::harbor::lighthouse_level(&data, &progress), 0.0);
        for _ in 0..story.chapters.len() {
            progress.finish_chapter(i);
        }
        assert_eq!(crate::harbor::lighthouse_level(&data, &progress), 1.0);
    }
}
