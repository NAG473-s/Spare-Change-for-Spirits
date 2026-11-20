//! Regulars: now and then a ghost that was served well likes the shop so
//! much that it comes back – by name, often for the ware it bought the first
//! time, and the more often it comes, the more often it leaves a tip
//! (`regulars` in `config.ron`, names `regular.name.<n>` in the catalogs).
//! They are kept in the save with the ghost book (`book.rs`).

use crate::book::Regular;
use crate::data::{GameData, GhostDef};
use crate::day_cycle::Progress;
use crate::i18n::Lang;
use crate::rng::Rng;
use crate::shop::ShopLayout;
use crate::ui::GameLog;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Memory {
    Purchase(String),
    Keepsake(String),
}

impl Memory {
    pub fn words(&self, lang: &Lang) -> String {
        match self {
            Self::Purchase(id) => {
                lang.f("regular.remembers_purchase", &[("item", &lang.product(id))])
            }
            Self::Keepsake(id) => {
                lang.f("regular.recognises_keepsake", &[("item", &lang.decor(id))])
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Encounter {
    night: u32,
    wish: String,
    memory: Memory,
}

impl Encounter {
    pub fn entry(&self, lang: &Lang) -> String {
        lang.f(
            "regular.encounter",
            &[
                ("night", &self.night),
                ("memory", &self.memory.words(lang)),
                ("wish", &lang.product(&self.wish)),
            ],
        )
    }
}

pub struct RegularVisit {
    pub index: usize,
    pub name: String,
    pub memory: Memory,
}

/// Recall a purchase, or recognise a keepsake that was on display during
/// an earlier visit. Keep this visit's words and wish together in the book.
pub fn greet(
    index: usize,
    progress: &mut Progress,
    wish: &str,
    data: &GameData,
    layout: &ShopLayout,
    lang: &Lang,
    log: &mut GameLog,
) -> RegularVisit {
    let regular = &mut progress.journal.regulars[index];
    let displayed = layout
        .cells
        .iter()
        .filter_map(|cell| data.decor(cell.decor.as_deref()?))
        .filter(|d| d.keepsake)
        .map(|d| &d.id);
    let last_memory = regular.encounters.last().map(|e| &e.memory);
    let recognised = displayed.clone().find(|id| {
        regular.seen_keepsakes.contains(id)
            && !matches!(last_memory, Some(Memory::Keepsake(last)) if last == *id)
    });
    let memory = match recognised {
        Some(id) => Memory::Keepsake(id.clone()),
        None => Memory::Purchase(
            regular
                .last_purchase
                .as_ref()
                .unwrap_or(&regular.favourite)
                .clone(),
        ),
    };
    for id in displayed {
        if !regular.seen_keepsakes.contains(id) {
            regular.seen_keepsakes.push(id.clone());
        }
    }
    regular.encounters.push(Encounter {
        night: progress.night,
        wish: wish.to_owned(),
        memory: memory.clone(),
    });
    let name = regular.name(lang);
    log.push(lang.f(
        "log.regular_back",
        &[("name", &name), ("visits", &regular.visits)],
    ));
    log.push(lang.f(
        "log.story_says",
        &[("name", &name), ("text", &memory.words(lang))],
    ));
    RegularVisit {
        index,
        name,
        memory,
    }
}

/// Is this arriving ghost of `ghost`'s kind one of the regulars? If so it
/// counts the visit and gives the regular's index.
pub fn returning(
    data: &GameData,
    progress: &mut Progress,
    ghost: &str,
    mut present: impl Iterator<Item = usize>,
    rng: &mut Rng,
) -> Option<usize> {
    let i = progress
        .journal
        .regulars
        .iter()
        .position(|r| r.ghost == ghost)?;
    let name = progress.journal.regulars[i].name;
    let collection_due = progress.orders.iter().any(|order| {
        order.night == progress.night
            && order.regular_name == Some(name)
            && matches!(
                order.state,
                crate::orders::OrderState::Open | crate::orders::OrderState::AtRegister
            )
    });
    if collection_due
        || present.any(|regular| regular == i)
        || !rng.chance(data.config.regulars.return_chance)
    {
        return None;
    }
    progress.journal.regulars[i].visits += 1;
    Some(i)
}

/// What a returning regular asks for: often its favourite, if that is a
/// ware of tonight; otherwise `other`.
pub fn wish(
    data: &GameData,
    regular: &Regular,
    tonight: &[&str],
    other: String,
    rng: &mut Rng,
) -> String {
    if tonight.contains(&regular.favourite.as_str())
        && rng.chance(data.config.regulars.favourite_chance)
    {
        regular.favourite.clone()
    } else {
        other
    }
}

/// A ghost served well may become a regular. Returns its journal index.
pub fn maybe_new(
    data: &GameData,
    progress: &mut Progress,
    def: &GhostDef,
    bought: &str,
    lang: &Lang,
    rng: &mut Rng,
) -> Option<usize> {
    let c = data.config.regulars;
    let regulars = &progress.journal.regulars;
    // Only ordinary customers; one regular of each kind.
    if def.spawn_weight <= 0.0
        || regulars.len() >= c.max
        || regulars.iter().any(|r| r.ghost == def.id)
        || !rng.chance(c.become_chance)
    {
        return None;
    }
    let names = lang.count("regular.name.") as u32;
    let name = (1..=names).find(|n| !regulars.iter().any(|r| r.name == *n))?;
    let regular = Regular {
        ghost: def.id.clone(),
        name,
        visits: 1,
        favourite: bought.to_string(),
        last_purchase: Some(bought.into()),
        seen_keepsakes: Vec::new(),
        encounters: Vec::new(),
    };
    let index = regulars.len();
    progress.journal.regulars.push(regular);
    Some(index)
}

/// The extra chance of a tip from a regular.
pub fn tip_bonus(data: &GameData, regular: Option<&Regular>) -> f32 {
    let c = data.config.regulars;
    regular.map_or(0.0, |r| {
        (r.visits as f32 * c.tip_per_visit).min(c.max_tip_bonus)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_return_recalls_a_real_purchase_or_a_keepsake_seen_on_an_earlier_visit() {
        let data = GameData::load();
        let mut progress = Progress::new(0);
        // Earlier profiles stored the genuine first purchase as the favourite.
        let old = r#"(ghost:"shy", name:1, visits:3, favourite:"lavender")"#;
        progress.journal.regulars.push(ron::from_str(old).unwrap());
        let lang = Lang::load("en");
        let mut layout = ShopLayout::new(4, 4);
        layout.cells[0].decor = Some("wedding_photo".into());
        layout.cells[1].decor = Some("wedding_photo".into());
        let mut log = GameLog::default();
        let first = greet(0, &mut progress, "marbles", &data, &layout, &lang, &mut log);
        assert_eq!(first.memory, Memory::Purchase("lavender".into()));
        // Serving a different ghost of the same kind cannot change the memory.
        progress.journal.served("shy", "tea_tin", None);
        progress.night += 1;
        let next = greet(
            0,
            &mut progress,
            "rain_jar",
            &data,
            &layout,
            &lang,
            &mut log,
        );
        assert_eq!(next.memory, Memory::Keepsake("wedding_photo".into()));
        progress.journal.served("shy", "rain_jar", Some(0));
        layout.cells[0].decor = None;
        layout.cells[1].decor = None;
        progress.night += 1;
        let last = greet(0, &mut progress, "tea_tin", &data, &layout, &lang, &mut log);
        assert_eq!(last.memory, Memory::Purchase("rain_jar".into()));

        let saved = ron::to_string(&progress.journal).unwrap();
        let loaded: crate::book::Journal = ron::from_str(&saved).unwrap();
        assert_eq!(progress.journal, loaded);
        let history = &loaded.regular("shy").unwrap().encounters;
        assert_eq!(history.len(), 3);
        assert_eq!(
            (history[1].night, history[1].wish.as_str()),
            (2, "rain_jar")
        );
        let entry = history[1].entry(&lang);
        assert!(entry.contains(&lang.decor("wedding_photo")));
        assert!(entry.contains(&lang.product("rain_jar")));
    }

    #[test]
    fn a_regular_comes_back_and_tips_more_often() {
        let data = GameData::load();
        let lang = Lang::load("en");
        let mut progress = Progress::new(0);
        let mut rng = Rng::new(3);
        let shy = &data.ghosts[data.ghost_index("shy").unwrap()];
        let index = (0..200)
            .find_map(|_| maybe_new(&data, &mut progress, shy, "lavender", &lang, &mut rng))
            .expect("becomes a regular sooner or later");
        let name = progress.journal.regulars[index].name(&lang);
        assert!(!name.is_empty() && !name.starts_with("regular."));
        // Only one regular of each kind.
        for _ in 0..50 {
            assert!(maybe_new(&data, &mut progress, shy, "tea_tin", &lang, &mut rng).is_none());
        }
        let back = (0..200)
            .find_map(|_| returning(&data, &mut progress, "shy", std::iter::empty(), &mut rng))
            .unwrap();
        let regular = &progress.journal.regulars[back];
        assert!(regular.visits >= 2);
        assert!(tip_bonus(&data, Some(regular)) > 0.0);
        assert!(returning(&data, &mut progress, "vain", std::iter::empty(), &mut rng).is_none());
    }

    #[test]
    fn a_regular_in_the_shop_or_due_to_collect_cannot_arrive_twice() {
        let mut data = GameData::load();
        data.config.regulars.return_chance = 1.0;
        let mut progress = Progress::new(0);
        progress
            .journal
            .regulars
            .push(ron::from_str(r#"(ghost:"shy",name:1,visits:1,favourite:"lavender")"#).unwrap());
        let mut rng = Rng::new(19);
        assert!(returning(&data, &mut progress, "shy", [0].into_iter(), &mut rng).is_none());
        progress
            .orders
            .push(crate::orders::Order::new("shy", "lavender", 1, Some(1)));
        assert!(returning(&data, &mut progress, "shy", std::iter::empty(), &mut rng).is_none());
        assert_eq!(progress.journal.regulars[0].visits, 1);
        progress.orders[0].state = crate::orders::OrderState::Collected;
        assert_eq!(
            returning(&data, &mut progress, "shy", std::iter::empty(), &mut rng),
            Some(0)
        );
    }
}
