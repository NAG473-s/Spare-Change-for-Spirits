//! Spare Change for Spirits – a shop management game for ghosts.
//!
//! Evening: order from the ferryman, stock the shelves, decorate the shop.
//! Night: serve ghosts, guess their wishes, hand out change.
//! Morning: accounts, unsold items fade.

// No console window in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Bevy systems take many parameters and complex queries by design.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod data;
mod day_cycle;
mod economy;
mod ghosts;
mod inventory;
mod register;
mod rng;
mod shop;
mod ui;

use bevy::prelude::*;

use data::GameData;
use day_cycle::Progress;
use economy::Drawer;

fn main() {
    let data = GameData::load();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Spare Change for Spirits".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.07)))
        .insert_resource(Drawer(data.config.start_coins))
        .insert_resource(Progress {
            night: 1,
            reputation: data.config.start_reputation,
        })
        .insert_resource(rng::Rng::from_time())
        .insert_resource(data)
        .add_plugins((
            ui::UiPlugin,
            day_cycle::DayCyclePlugin,
            inventory::InventoryPlugin,
            shop::ShopPlugin,
            ghosts::GhostPlugin,
            register::RegisterPlugin,
        ))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .run();
}
