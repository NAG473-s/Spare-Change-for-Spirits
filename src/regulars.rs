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

/// Is this arriving ghost of `ghost`'s kind one of the regulars? If so it
/// counts the visit and gives the regular's index.
pub fn returning(
    data: &GameData,
    progress: &mut Progress,
    ghost: &str,
    rng: &mut Rng,
) -> Option<usize> {
    let i = progress
        .journal
        .regulars
        .iter()
        .position(|r| r.ghost == ghost)?;
    if !rng.chance(data.config.regulars.return_chance) {
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

/// A ghost served well may become a regular. Gives its name for the log.
pub fn maybe_new(
    data: &GameData,
    progress: &mut Progress,
    def: &GhostDef,
    bought: &str,
    lang: &Lang,
    rng: &mut Rng,
) -> Option<String> {
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
    };
    let text = regular.name(lang);
    progress.journal.regulars.push(regular);
    Some(text)
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
    fn a_regular_comes_back_and_tips_more_often() {
        let data = GameData::load();
        let lang = Lang::load("en");
        let mut progress = Progress::new(0);
        let mut rng = Rng::new(3);
        let shy = &data.ghosts[data.ghost_index("shy").unwrap()];
        let name = (0..200)
            .find_map(|_| maybe_new(&data, &mut progress, shy, "lavender", &lang, &mut rng))
            .expect("becomes a regular sooner or later");
        assert!(!name.is_empty() && !name.starts_with("regular."));
        // Only one regular of each kind.
        for _ in 0..50 {
            assert!(maybe_new(&data, &mut progress, shy, "tea_tin", &lang, &mut rng).is_none());
        }
        let back = (0..200)
            .find_map(|_| returning(&data, &mut progress, "shy", &mut rng))
            .unwrap();
        let regular = &progress.journal.regulars[back];
        assert!(regular.visits >= 2);
        assert!(tip_bonus(&data, Some(regular)) > 0.0);
        assert!(returning(&data, &mut progress, "vain", &mut rng).is_none());
    }
}
