//! Spare Change for Spirits – a shop management game for ghosts.
//!
//! Evening: order from the ferryman, stock the shelves, decorate the shop.
//! Night: serve ghosts, guess their wishes, hand out change.
//! Morning: accounts, unsold items fade.

// No console window in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Bevy systems take many parameters and complex queries by design.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod accounts;
mod achievements;
mod apprentice;
mod audio;
mod book;
mod contrast;
mod critters;
mod data;
mod day_cycle;
mod economy;
mod ending;
mod errands;
mod ferry;
mod finale;
mod fx;
mod gamepad;
mod ghosts;
mod guide;
mod handoff;
mod harbor;
mod i18n;
mod incidents;
mod inventory;
mod keynav;
mod lanterns;
mod loan;
mod market;
mod menu;
mod orders;
mod pfote;
mod playtest;
mod postcards;
mod props;
mod register;
mod regulars;
mod rng;
mod save;
mod scene;
mod sets;
mod shop;
mod showcase;
mod sign;
mod sim;
mod story_film;
mod title;
mod tooltip;
mod tower_clock;
mod trailer;
mod ui;
mod upgrades;
mod weather;
mod winter;
mod workbench;

use bevy::prelude::*;

use data::GameData;
use day_cycle::Progress;
use economy::Drawer;

fn main() {
    let data = GameData::load();
    // `--simulate [runs]`: the balancing simulation, without a window.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--simulate") {
        let runs = args.get(i + 1).and_then(|r| r.parse().ok()).unwrap_or(500);
        sim::run(&data, runs);
        return;
    }
    if args.iter().any(|arg| arg == "--check-demo-handoff") {
        handoff::run(data);
        return;
    }
    let automatic = i18n::AutomaticLanguage::at_launch();
    let lang = i18n::Lang::from_preference(save::Settings::load().language.as_deref(), &automatic);

    let plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Spare Change for Spirits".into(),
            name: Some("spare-change-for-spirits".into()),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    });
    let mut app = App::new();
    app.add_plugins(plugins)
        // Clicking on 3D objects (floor tiles, furniture).
        .add_plugins(MeshPickingPlugin)
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.07)))
        .insert_resource(Drawer(data.config.start_coins))
        .insert_resource(Progress::new(data.config.start_reputation))
        .insert_resource(rng::Rng::from_time())
        .insert_resource(data)
        .insert_resource(automatic)
        .insert_resource(lang)
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
            ferry::FerryPlugin,
            harbor::HarborPlugin,
            accounts::AccountsPlugin,
        ))
        .add_plugins((
            sign::SignPlugin,
            props::PropsPlugin,
            critters::CrittersPlugin,
            tooltip::TooltipPlugin,
            market::MarketPlugin,
            loan::LoanPlugin,
            playtest::PlaytestPlugin,
            upgrades::UpgradesPlugin,
            incidents::IncidentsPlugin,
            workbench::WorkbenchPlugin,
            winter::WinterPlugin,
            achievements::AchievementsPlugin,
        ))
        .add_plugins((
            trailer::TrailerPlugin,
            weather::WeatherPlugin,
            showcase::ShowcasePlugin,
            apprentice::ApprenticePlugin,
            gamepad::GamepadPlugin,
            finale::FinalePlugin,
            errands::ErrandsPlugin,
            tower_clock::TowerClockPlugin,
            ending::EndingPlugin,
            story_film::StoryFilmPlugin,
            regulars::RegularsPlugin,
            lanterns::LanternsPlugin,
            keynav::KeyNavPlugin,
            contrast::ContrastPlugin,
            postcards::PostcardsPlugin,
        ))
        .add_plugins((pfote::PfotePlugin,));
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    app.add_systems(Update, set_window_icon);
    app.run();
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn set_window_icon(
    mut created: MessageReader<bevy::window::WindowCreated>,
    _main_thread: bevy::ecs::system::NonSendMarker,
) {
    use bevy::winit::WINIT_WINDOWS;
    use winit::window::Icon;

    for event in created.read() {
        WINIT_WINDOWS.with_borrow(|windows| {
            let Some(window) = windows.get_window(event.window) else {
                return;
            };
            #[cfg(target_os = "linux")]
            {
                let png = include_bytes!("../art/icons/spare-change-for-spirits.png");
                let pixels = image::load_from_memory_with_format(png, image::ImageFormat::Png)
                    .expect("bundled application icon is a PNG")
                    .resize_exact(128, 128, image::imageops::FilterType::Lanczos3)
                    .into_rgba8();
                let icon = Icon::from_rgba(pixels.into_raw(), 128, 128)
                    .expect("128 by 128 RGBA application icon");
                window.set_window_icon(Some(icon));
            }
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::{IconExtWindows, WindowExtWindows};
                // Use the matching small and large images from the executable's icon group.
                window.set_window_icon(Icon::from_resource(1, Some((32, 32).into())).ok());
                window.set_taskbar_icon(Icon::from_resource(1, Some((64, 64).into())).ok());
            }
        });
    }
}
