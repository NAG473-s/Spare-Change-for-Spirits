//! Spare Change for Spirits – a shop management game for ghosts.
//!
//! Evening: order from the ferryman, stock the shelves, decorate the shop.
//! Night: serve ghosts, guess their wishes, hand out change.
//! Morning: accounts, unsold items fade.

// No console window in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Bevy systems take many parameters and complex queries by design.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod audio;
mod data;
mod day_cycle;
mod economy;
mod fx;
mod ghosts;
mod i18n;
mod inventory;
mod menu;
mod register;
mod rng;
mod save;
mod scene;
mod shop;
mod title;
mod ui;

use bevy::prelude::*;

use data::GameData;
use day_cycle::Progress;
use economy::Drawer;

fn main() {
    let data = GameData::load();
    // The saved language, else the system's, else English.
    let code = save::Settings::load()
        .language
        .or_else(|| i18n::detect(&i18n::languages()))
        .unwrap_or_else(|| i18n::SOURCE.to_string());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Spare Change for Spirits".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        // Clicking on 3D objects (floor tiles, furniture).
        .add_plugins(MeshPickingPlugin)
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.07)))
        .insert_resource(Drawer(data.config.start_coins))
        .insert_resource(Progress {
            night: 1,
            reputation: data.config.start_reputation,
            story_chapter: 0,
        })
        .insert_resource(rng::Rng::from_time())
        .insert_resource(data)
        .insert_resource(i18n::Lang::load(&code))
        .add_plugins((
            ui::UiPlugin,
            day_cycle::DayCyclePlugin,
            inventory::InventoryPlugin,
            shop::ShopPlugin,
            ghosts::GhostPlugin,
            register::RegisterPlugin,
            save::SavePlugin,
            menu::MenuPlugin,
            fx::FxPlugin,
            audio::SoundPlugin,
            scene::ScenePlugin,
            title::TitlePlugin,
        ))
        .run();
}
