//! Exercises the shipped demo and the full game's Continue action in separate processes.
//! The runner provides a fresh, isolated profile. No renderer or window is needed.
use crate::{
    accounts,
    data::GameData,
    day_cycle::{self, Phase, Progress},
    economy::{Coin, Drawer},
    ferry,
    fx::Cue,
    ghosts::{self, Ghost, GhostState, Queue},
    guide::FirstSale,
    harbor::Location,
    i18n::{AutomaticLanguage, Lang},
    inventory::{self, Stock},
    loan,
    market::MarketDay,
    menu, register,
    rng::Rng,
    save,
    scene::ClockOverride,
    shop::{self, ShopLayout, Storeroom, Tool},
    sign,
    story_film::PlayStoryFilm,
    ui::{Action, GameLog, UiFont},
    upgrades::Upgrades,
};
use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use std::time::Duration;

pub(crate) fn run(data: GameData) {
    let profile =
        std::env::var_os(save::SAVE_DIR_ENV).expect("the handoff check needs an isolated profile");
    assert!(
        std::path::Path::new(&profile)
            .join(".handoff-check")
            .is_file(),
        "missing test profile marker"
    );
    let limit = data.config.demo_nights;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin, AssetPlugin::default()))
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            250,
        )))
        .insert_resource(Lang::load("en"))
        .insert_resource(AutomaticLanguage::at_launch())
        .insert_resource(Rng::new(153))
        .insert_resource(Drawer(data.config.start_coins))
        .insert_resource(Progress::new(data.config.start_reputation))
        .insert_resource(ShopLayout::new(
            data.config.grid_cols,
            data.config.grid_rows,
        ))
        .insert_resource(data)
        .init_resource::<UiFont>()
        .init_resource::<GameLog>()
        .init_resource::<Storeroom>()
        .init_resource::<Upgrades>()
        .init_resource::<Tool>()
        .init_resource::<MarketDay>()
        .init_resource::<ferry::BoughtTonight>()
        .init_resource::<accounts::Accounts>()
        .init_resource::<menu::LanguageMenu>()
        .init_resource::<menu::SaveMenu>()
        .init_resource::<Location>()
        .init_resource::<ClockOverride>()
        .init_resource::<sign::Opening>()
        .add_message::<Action>()
        .add_message::<Cue>()
        .add_message::<ghosts::Finale>()
        .add_message::<PlayStoryFilm>()
        .add_plugins((
            day_cycle::DayCyclePlugin,
            inventory::InventoryPlugin,
            register::RegisterPlugin,
            loan::LoanPlugin,
            save::SavePlugin,
        ))
        .add_systems(
            Update,
            menu::main_menu_actions.run_if(in_state(Phase::Menu)),
        )
        .add_systems(
            Update,
            (
                shop::select_tool,
                shop::use_tool_anywhere,
                sign::start_opening,
                sign::run_opening,
            )
                .chain()
                .run_if(in_state(Phase::Evening)),
        )
        .add_systems(
            OnEnter(Phase::Night),
            (
                accounts::open_register,
                |mut opening: ResMut<sign::Opening>| opening.0 = None,
            ),
        )
        .add_systems(OnEnter(Phase::Evening), accounts::open_books)
        .add_systems(OnEnter(Phase::Day), accounts::open_books)
        .add_systems(
            OnEnter(Phase::Morning),
            accounts::close_books.after(loan::repay),
        );
    app.add_systems(
        Update,
        shop::clear_sold_out.run_if(resource_changed::<Stock>),
    );
    ferry::install_trade(&mut app);
    ghosts::install_headless_customers(&mut app);
    app.finish();
    app.cleanup();
    app.update();

    if cfg!(feature = "demo") {
        assert!(
            save::latest_save(1).is_none(),
            "demo requires an empty profile"
        );
        action(&mut app, Action::NewGameIn(1));
        for night in 1..=limit {
            reach(&mut app, Phase::Evening);
            assert_eq!(app.world().resource::<Progress>().night, night);
            if night == 1 {
                action(&mut app, Action::BuyCrate("lavender".into()));
                action(&mut app, Action::ToolStock("lavender".into()));
                action(&mut app, Action::UseToolAnywhere);
                assert!(
                    app.world()
                        .resource::<ShopLayout>()
                        .displayed_products()
                        .contains(&"lavender".into())
                );
            }
            if night > 1 {
                action(&mut app, Action::BuyCrate("marbles".into()));
                if night == limit {
                    action(&mut app, Action::BuyCrate("tea_tin".into()));
                }
            }
            action(&mut app, Action::StartNight);
            reach(&mut app, Phase::Night);
            reach(&mut app, Phase::Morning);
            let (_, dawn) = save::latest_save(1).expect("dawn autosave");
            assert_eq!(dawn.night, night + 1);
            assert_eq!(dawn.journal.first_sale, FirstSale::Complete);
            assert!(!dawn.journal.book.is_empty());
            if night == limit {
                assert!(dawn.stock.values().any(|units| !units.is_empty()));
                assert_eq!(dawn.accounts.night, night);
                assert_eq!(dawn.accounts.closing, dawn.drawer.total());
            }
            println!(
                "Demo night {night} complete, {} ghost entries, {} buttons in the drawer",
                dawn.journal.book.len(),
                dawn.drawer.total()
            );
            action(&mut app, Action::NextEvening);
            if night < limit {
                reach(&mut app, Phase::Day);
                action(&mut app, Action::EndDay);
            }
        }
        reach(&mut app, Phase::DemoEnd);
        assert!(save::newest_save(limit).is_none());
    } else {
        let (_, _, before) = save::newest_save(limit).expect("demo's completed profile");
        assert!(before.finished && before.night == limit + 1);
        action(&mut app, Action::Continue);
        reach(&mut app, Phase::Evening);
        let world = app.world();
        let progress = world.resource::<Progress>();
        assert_eq!(progress.night, before.night);
        assert_eq!(progress.journal, before.journal);
        assert_eq!(progress.reputation, before.reputation);
        assert_eq!(progress.story, before.story);
        assert_eq!(progress.orders, before.orders);
        assert_eq!(world.resource::<Drawer>().0, before.drawer);
        assert_eq!(world.resource::<Stock>().units(), &before.stock);
        assert_eq!(world.resource::<ShopLayout>().cells, before.cells);
        action(&mut app, Action::StartNight);
        reach(&mut app, Phase::Night);
        println!(
            "Full game resumed the demo profile and opened night {}",
            before.night
        );
    }
}

fn action(app: &mut App, action: Action) {
    app.world_mut().write_message(action);
    app.update();
}

fn reach(app: &mut App, wanted: Phase) {
    for _ in 0..5000 {
        let world = app.world();
        let phase = *world.resource::<State<Phase>>().get();
        if phase == wanted {
            return;
        }
        if phase == Phase::Night {
            let next = customer_action(world);
            if let Some(next) = next {
                app.world_mut().write_message(next);
            }
        }
        app.update();
    }
    panic!(
        "failed to reach {wanted:?} from {:?}",
        app.world().resource::<State<Phase>>().get()
    );
}

fn customer_action(world: &World) -> Option<Action> {
    let register = world.resource::<register::Register>();
    if let Some(sale) = &register.sale {
        let left = sale.change_due() - sale.given.total();
        if left == 0 {
            return Some(Action::GiveChange);
        }
        let drawer = &world.resource::<Drawer>().0;
        if !drawer.can_give(sale.change_due()) {
            return Some(Action::NoChange);
        }
        return Coin::DESCENDING
            .into_iter()
            .find(|coin| {
                let mut available = *drawer;
                available.remove(&sale.given);
                if coin.value() > left || available.get(*coin) == 0 {
                    return false;
                }
                let mut one = crate::economy::Purse::default();
                one.add(*coin, 1);
                available.remove(&one);
                available.can_give(left - coin.value())
            })
            .map(Action::AddChange);
    }
    if register.question.is_some() {
        return Some(Action::RefuseSecondPayment);
    }
    let ghost = world.get::<Ghost>(world.resource::<Queue>().front()?)?;
    if ghost.state != GhostState::Waiting {
        return None;
    }
    if world.resource::<Stock>().count(&ghost.wish) > 0 {
        Some(Action::Offer(ghost.wish.clone()))
    } else if ghost
        .chapter
        .is_some_and(|(i, _)| world.resource::<Progress>().parcels.contains(&i))
    {
        Some(Action::HandParcel)
    } else {
        Some(Action::Decline)
    }
}
