//! Wrapped purchases travel between the trader's sample and the player's willow basket.
use super::{Kit, MARKET, MarketDay, WareKind};
use crate::save::Settings;
use crate::shop::model_or_box;
use bevy::prelude::*;
use std::collections::BTreeMap;

const BASKET: Vec3 = Vec3::new(1.1, 0.0, 4.6);
const TURN: f32 = -0.18;
const SLOTS: usize = 4;
const FLIGHT_TIME: f32 = 0.90;
const PARCEL: &str = "models/market/wrapped_parcel.glb";

#[derive(Component)]
pub(super) struct Packed(usize);

/// Slot is the number of other things in the basket at the time of the handoff.
/// The same place is used for the outward trip if the player immediately returns a purchase.
#[derive(Message)]
pub(super) struct Trade {
    pub slot: usize,
    pub sample: Vec3,
    pub bought: bool,
}

#[derive(Component)]
pub(super) struct Flight {
    slot: usize,
    bought: bool,
    from: Vec3,
    to: Vec3,
    age: f32,
}

pub(super) fn count(day: &MarketDay) -> usize {
    day.bought.len() + day.rare_sold.len() + usize::from(day.hourglass)
}

pub(super) fn tangible(kind: &WareKind) -> bool {
    matches!(
        kind,
        WareKind::Decor(_) | WareKind::Rare(_) | WareKind::Single(_) | WareKind::Hourglass
    )
}

pub(super) fn sold_piece(day: &MarketDay, vendor: usize, kind: &WareKind) -> bool {
    match kind {
        WareKind::Rare(id) | WareKind::Single(id) => {
            day.rare_sold.iter().any(|(v, p)| *v == vendor && p == id)
        }
        WareKind::Hourglass => day.hourglass,
        _ => false,
    }
}

fn place(slot: usize) -> Transform {
    let slot = slot.min(SLOTS - 1);
    let (x, y, z, yaw) = [
        (-0.18, 0.36, 0.06, -0.24),
        (0.22, 0.37, -0.04, 0.40),
        (-0.22, 0.55, 0.01, 0.21),
        (0.15, 0.57, 0.07, -0.32),
    ][slot];
    Transform::from_translation(MARKET + BASKET + Quat::from_rotation_y(TURN) * Vec3::new(x, y, z))
        .with_rotation(Quat::from_rotation_y(TURN + yaw))
}

fn parcel(k: &mut Kit, at: Transform) -> Entity {
    let root = k.node(None, at);
    let model = model_or_box(
        &mut k.commands,
        k.asset_server,
        k.meshes,
        k.materials,
        PARCEL,
        Vec3::new(0.40, 0.245, 0.29),
        Color::srgb(0.74, 0.56, 0.34),
    );
    k.commands
        .entity(model)
        .insert((ChildOf(root), Pickable::IGNORE));
    root
}

pub(super) fn spawn(k: &mut Kit) {
    let socket = k.node(
        None,
        Transform::from_translation(MARKET + BASKET).with_rotation(Quat::from_rotation_y(TURN)),
    );
    k.commands
        .entity(socket)
        .insert(DespawnOnExit(crate::day_cycle::Phase::Day));
    let basket = model_or_box(
        &mut k.commands,
        k.asset_server,
        k.meshes,
        k.materials,
        "models/market/shopping_basket.glb",
        Vec3::new(1.48, 0.99, 0.9),
        Color::srgb(0.62, 0.46, 0.29),
    );
    k.commands
        .entity(basket)
        .insert((ChildOf(socket), Pickable::IGNORE));
    for slot in 0..SLOTS {
        let root = parcel(k, place(slot));
        k.commands.entity(root).insert((
            Packed(slot),
            Visibility::Hidden,
            DespawnOnExit(crate::day_cycle::Phase::Day),
        ));
    }
}

pub(super) fn handoffs(
    commands: Commands,
    mut events: MessageReader<Trade>,
    settings: Res<Settings>,
    flights: Query<(Entity, &Flight, &Transform)>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut k = Kit {
        commands,
        asset_server: &assets,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    // A slot can be filled, returned and refilled before the same frame is painted.
    let mut latest = BTreeMap::new();
    for trade in events.read() {
        latest.insert(trade.slot, trade);
    }
    for (slot, trade) in latest {
        let packed_at = place(slot).translation;
        let (mut from, to) = if trade.bought {
            (trade.sample, packed_at)
        } else {
            (packed_at, trade.sample)
        };
        if let Some((old, _, at)) = flights.iter().find(|(_, f, _)| f.slot == slot) {
            from = at.translation;
            k.commands.entity(old).despawn();
        }
        if !settings.reduced_motion {
            let root = parcel(&mut k, Transform::from_translation(from));
            k.commands.entity(root).insert((
                Flight {
                    slot,
                    bought: trade.bought,
                    from,
                    to,
                    age: 0.0,
                },
                DespawnOnExit(crate::day_cycle::Phase::Day),
            ));
        }
    }
}

pub(super) fn arrange(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<Settings>,
    day: Res<MarketDay>,
    mut flights: Query<(Entity, &mut Flight, &mut Transform)>,
    mut packed: Query<(&Packed, &mut Visibility)>,
) {
    let mut arriving = [false; SLOTS];
    for (entity, mut flight, mut at) in &mut flights {
        flight.age += time.delta_secs();
        let t = (flight.age / FLIGHT_TIME).min(1.0);
        if t >= 1.0 || settings.reduced_motion {
            commands.entity(entity).despawn();
            continue;
        }
        if flight.bought
            && let Some(slot) = arriving.get_mut(flight.slot)
        {
            *slot = true;
        }
        let k = t * t * (3.0 - 2.0 * t);
        let arc = 4.0 * k * (1.0 - k);
        at.translation = flight.from.lerp(flight.to, k) + Vec3::Y * (1.28 * arc);
        at.rotation = place(flight.slot).rotation * Quat::from_rotation_z(0.34 * arc);
    }
    let total = count(&day);
    for (Packed(slot), mut v) in &mut packed {
        let visible = if *slot < total && !arriving[*slot] {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        v.set_if_neq(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restored_purchases_fill_the_basket_and_a_return_frees_its_slot() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Settings>()
            .init_resource::<MarketDay>()
            .add_systems(Update, arrange);
        let day = &mut *app.world_mut().resource_mut::<MarketDay>();
        day.bought.push("fern".into());
        day.rare_sold.push((2, "rain_jar".into()));
        let slot = app.world_mut().spawn((Packed(1), Visibility::Hidden)).id();
        let kind = WareKind::Rare("rain_jar".into());
        assert!(sold_piece(app.world().resource::<MarketDay>(), 2, &kind));
        assert!(!sold_piece(app.world().resource::<MarketDay>(), 1, &kind));
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(slot).unwrap(),
            Visibility::Inherited
        );
        let flying = app
            .world_mut()
            .spawn((
                Flight {
                    slot: 1,
                    bought: true,
                    from: MARKET,
                    to: place(1).translation,
                    age: 0.,
                },
                Transform::default(),
            ))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(slot).unwrap(),
            Visibility::Hidden
        );
        app.world_mut().resource_mut::<Settings>().reduced_motion = true;
        app.update();
        assert!(!app.world().entities().contains(flying));
        assert_eq!(
            *app.world().get::<Visibility>(slot).unwrap(),
            Visibility::Inherited
        );
        app.world_mut().resource_mut::<MarketDay>().bought.clear();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(slot).unwrap(),
            Visibility::Hidden
        );
    }
}
