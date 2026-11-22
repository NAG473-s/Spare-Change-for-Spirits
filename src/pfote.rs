//! Pfote's favourite rug. Once Lotte's story is finished, her ghost dog
//! lives in the shop (`critters.rs`). As soon as a rug lies in the shop he
//! picks one as his favourite spot (`companion.rugs` in `config.ron`, the
//! first listed that lies there) and curls up on it from then on; without it
//! he keeps to his corner.
//!
//! Now and then Lotte drops by during the night (`companion.visit_chance`):
//! she goes to his rug, he jumps up and wags his tail, and she leaves again.
//! The log says so in words, so the moment also reads with the sound off.

use bevy::prelude::*;
use serde::Deserialize;

use crate::data::GameData;
use crate::day_cycle::{NightClock, Phase, Progress};
use crate::ghosts::{Ghost, GhostState, Visiting, Visitor};
use crate::i18n::Lang;
use crate::rng::Rng;
use crate::scene::floor;
use crate::shop::ShopLayout;
use crate::ui::GameLog;

pub struct PfotePlugin;

impl Plugin for PfotePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VisitTonight>()
            .init_resource::<Greeting>()
            .add_systems(OnEnter(Phase::Night), plan_visit)
            .add_systems(
                Update,
                (
                    choose_rug.run_if(in_state(Phase::Evening)),
                    (lie_on_rug, crate::critters::greet_hop).chain(),
                    (send_visitor, greet).run_if(in_state(Phase::Night)),
                ),
            );
    }
}

/// `companion` in config.ron.
#[derive(Deserialize, Clone, Debug)]
pub struct CompanionConfig {
    /// Rugs (decor ids) the companion may choose, the most liked first.
    pub rugs: Vec<String>,
    /// Chance a night that its owner drops by, and when (fraction of the night).
    pub visit_chance: f32,
    pub visit_at: (f32, f32),
}

impl Default for CompanionConfig {
    fn default() -> Self {
        Self {
            rugs: vec!["fur_rug".into(), "rug".into()],
            visit_chance: 0.35,
            visit_at: (0.15, 0.6),
        }
    }
}

/// The companion that lives in the shop: (its story index, owner ghost id).
pub fn companion(data: &GameData, progress: &Progress) -> Option<(usize, String)> {
    data.stories
        .iter()
        .enumerate()
        .find(|(i, s)| s.companion.is_some() && progress.chapter(*i) >= s.chapters.len())
        .map(|(i, s)| (i, s.ghost.clone()))
}

/// The rug the companion would choose now: the first listed that lies in
/// the shop.
pub fn rug_to_choose(data: &GameData, layout: &ShopLayout) -> Option<String> {
    data.config
        .companion
        .rugs
        .iter()
        .find(|r| {
            layout
                .cells
                .iter()
                .any(|c| c.decor.as_deref() == Some(r.as_str()))
        })
        .cloned()
}

/// Where the companion lies: on its favourite rug if it lies in the shop.
pub fn rug_cell(progress: &Progress, layout: &ShopLayout) -> Option<usize> {
    let rug = progress.journal.companion_rug.as_deref()?;
    layout
        .cells
        .iter()
        .position(|c| c.decor.as_deref() == Some(rug))
}

/// The evening: the companion picks its favourite rug once.
fn choose_rug(
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    lang: Res<Lang>,
    mut progress: ResMut<Progress>,
    mut log: ResMut<GameLog>,
) {
    if progress.journal.companion_rug.is_some() || companion(&data, &progress).is_none() {
        return;
    }
    let Some(rug) = rug_to_choose(&data, &layout) else {
        return;
    };
    log.push(lang.f("log.pfote_rug", &[("rug", &lang.decor(&rug))]));
    progress.journal.companion_rug = Some(rug);
}

/// The companion (`critters::Companion`, found by its marker) lies on its rug.
fn lie_on_rug(
    progress: Res<Progress>,
    layout: Option<Res<ShopLayout>>,
    mut dogs: Query<&mut Transform, With<crate::critters::Companion>>,
) {
    let Some(layout) = layout else {
        return;
    };
    let spot = rug_cell(&progress, &layout)
        .map(|c| floor(layout.cell_pos(c), 0.0) + RUG_OFFSET)
        .unwrap_or(crate::critters::SHOP_DOG);
    for mut transform in &mut dogs {
        if transform.translation != spot {
            transform.translation = spot;
        }
    }
}

/// A little off the middle of the square, so furniture on it does not hide him.
const RUG_OFFSET: Vec3 = Vec3::new(0.15, 0.03, 0.25);

/// When tonight's visit comes (fraction of the night), if it comes.
#[derive(Resource, Default)]
struct VisitTonight(Option<f32>);

fn plan_visit(
    data: Res<GameData>,
    progress: Res<Progress>,
    layout: Res<ShopLayout>,
    mut rng: ResMut<Rng>,
    mut visit: ResMut<VisitTonight>,
) {
    let c = &data.config.companion;
    visit.0 = (companion(&data, &progress).is_some()
        && rug_cell(&progress, &layout).is_some()
        && rng.chance(c.visit_chance))
    .then(|| rng.range(c.visit_at.0, c.visit_at.1));
}

fn send_visitor(
    data: Res<GameData>,
    progress: Res<Progress>,
    layout: Res<ShopLayout>,
    clock: Res<NightClock>,
    mut visit: ResMut<VisitTonight>,
    mut visitors: MessageWriter<Visitor>,
) {
    let Some(at) = visit.0 else {
        return;
    };
    if clock.elapsed < at * clock.length {
        return;
    }
    visit.0 = None;
    let (Some((_, owner)), Some(cell)) =
        (companion(&data, &progress), rug_cell(&progress, &layout))
    else {
        return;
    };
    // She stands beside the rug, on the camera side.
    visitors.write(Visitor {
        ghost: owner,
        spot: layout.cell_pos(cell) + Vec2::new(-0.35, 0.55),
    });
}

/// Seconds the companion has been greeting, while it does.
#[derive(Resource, Default)]
pub struct Greeting(pub Option<f32>);

/// The owner has reached the rug: the companion jumps up and wags its tail.
fn greet(
    time: Res<Time>,
    data: Res<GameData>,
    lang: Res<Lang>,
    progress: Res<Progress>,
    visitors: Query<&Ghost, With<Visiting>>,
    mut greeting: ResMut<Greeting>,
    mut log: ResMut<GameLog>,
) {
    let there = visitors.iter().any(|g| g.state == GhostState::Browsing);
    match (greeting.0, there) {
        (None, true) => {
            greeting.0 = Some(0.0);
            if let (Some((_, owner)), Some(rug)) = (
                companion(&data, &progress),
                progress.journal.companion_rug.as_deref(),
            ) {
                log.push(lang.f(
                    "log.pfote_greets",
                    &[("who", &lang.ghost(&owner)), ("rug", &lang.decor(rug))],
                ));
            }
        }
        (Some(t), true) => greeting.0 = Some(t + time.delta_secs()),
        (Some(_), false) => greeting.0 = None,
        (None, false) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfote_prefers_the_fur_rug_and_lies_where_it_is() {
        let data = GameData::load();
        let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        layout.cells.fill(Default::default());
        assert_eq!(rug_to_choose(&data, &layout), None);
        layout.cells[3].decor = Some("rug".into());
        assert_eq!(rug_to_choose(&data, &layout).as_deref(), Some("rug"));
        layout.cells[5].decor = Some("fur_rug".into());
        assert_eq!(rug_to_choose(&data, &layout).as_deref(), Some("fur_rug"));
        for rug in &data.config.companion.rugs {
            assert!(data.decor(rug).is_some(), "{rug} is furniture");
        }
        let mut progress = Progress::new(0);
        assert_eq!(rug_cell(&progress, &layout), None);
        progress.journal.companion_rug = Some("rug".into());
        assert_eq!(rug_cell(&progress, &layout), Some(3));
        // Taken away: he goes back to his corner, and still likes that rug.
        layout.cells[3].decor = None;
        assert_eq!(rug_cell(&progress, &layout), None);
    }

    #[test]
    #[cfg(not(feature = "demo"))]
    fn only_lotte_s_finished_story_brings_a_companion() {
        let data = GameData::load();
        let mut progress = Progress::new(0);
        assert_eq!(companion(&data, &progress), None);
        let (i, story) = data
            .stories
            .iter()
            .enumerate()
            .find(|(_, s)| s.companion.is_some())
            .unwrap();
        for _ in 0..story.chapters.len() {
            progress.finish_chapter(i);
        }
        assert_eq!(companion(&data, &progress), Some((i, "lotte".into())));
    }
}
