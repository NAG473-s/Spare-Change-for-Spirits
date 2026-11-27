//! The two faces of the shop's letter slot. Dawn already put the actual cards in the book.
use super::{
    residents::{self, Job, Resident, ease},
    rounds::{Art, hand},
};
use crate::{
    day_cycle::{Phase, Progress},
    harbor::Location,
    save::Settings,
    shop::entrance::DoorSwing,
    ui::Action,
};
use bevy::{light::NotShadowCaster, picking::pointer::PointerButton, prelude::*};

const INNER_SLOT: Vec3 = Vec3::new(-0.44, 0.92, 0.102);
const STREET_SLOT: Vec3 = Vec3::new(0., 0.92, 0.142);
const TUCK: Vec3 = Vec3::new(0.01, 0.070, 0.042);
#[derive(Component)]
pub(super) struct PostGate;
#[derive(Component, PartialEq)]
enum Face {
    Shop,
    Street,
}

#[derive(Resource, Default, PartialEq, Eq)]
pub(super) enum Round {
    #[default]
    Idle,
    Approaching,
    Posted,
}

impl Round {
    pub(super) fn carrying_delivery(&self) -> bool {
        *self != Self::Idle
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Round>()
        .add_systems(PostStartup, setup)
        .add_systems(OnEnter(Phase::Day), begin_round)
        .add_systems(OnExit(Phase::Day), |mut round: ResMut<Round>| {
            *round = Round::Idle
        })
        .add_systems(
            Update,
            display
                .after(residents::walk)
                .run_if(crate::menu::shop_unpaused),
        );
}

fn begin_round(
    progress: Res<Progress>,
    settings: Res<Settings>,
    mut round: ResMut<Round>,
    mut residents: Query<&mut Resident>,
) {
    *round = Round::Idle;
    if progress
        .journal
        .postcards
        .iter()
        .any(|p| p.in_mailbox && p.night.checked_add(1) == Some(progress.night))
    {
        if let Some(mut carrier) = residents.iter_mut().find(|r| r.job == Job::Post)
            && !settings.reduced_motion
        {
            carrier.approach_mailbox();
            *round = Round::Approaching;
        } else {
            *round = Round::Posted;
        }
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    door: Single<Entity, With<DoorSwing>>,
    gate: Query<Entity, With<PostGate>>,
) {
    let mut art = Art {
        commands: &mut commands,
        assets: &assets,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let faces = std::iter::once((*door, Face::Shop, INNER_SLOT))
        .chain(gate.iter().map(|e| (e, Face::Street, STREET_SLOT)));
    for (parent, face, slot) in faces {
        let plate = art.piece(
            "door_letter_slot",
            Vec3::new(0.47, 0.14, 0.06),
            Color::srgb(0.67, 0.53, 0.29),
        );
        art.commands.entity(plate).insert((
            Transform::from_translation(slot),
            Visibility::Inherited,
            ChildOf(parent),
        ));
        let letter = art.piece(
            "round_letter",
            Vec3::new(0.33, 0.23, 0.04),
            Color::srgb(0.92, 0.85, 0.69),
        );
        art.commands
            .entity(letter)
            .insert((face, ChildOf(parent)))
            .observe(pick_up);
        art.commands.spawn((
            Mesh3d(art.meshes.add(Cuboid::new(0.43, 0.30, 0.10))),
            MeshMaterial3d(art.materials.add(StandardMaterial {
                base_color: Color::NONE,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(0., 0.12, 0.04),
            NotShadowCaster,
            ChildOf(letter),
        ));
    }
}

fn pick_up(mut click: On<PointerClick>, mut actions: MessageWriter<Action>) {
    if click.button == PointerButton::Primary {
        click.propagate(false);
        actions.write(Action::ReadDelivery);
    }
}

fn display(
    phase: Res<State<Phase>>,
    location: Res<Location>,
    progress: Res<Progress>,
    settings: Res<Settings>,
    mut round: ResMut<Round>,
    residents: Query<&Resident>,
    gate: Query<&GlobalTransform, With<PostGate>>,
    mut envelopes: Query<(&Face, &mut Transform, &mut Visibility)>,
) {
    let pending =
        *phase.get() != Phase::Menu && progress.journal.postcards.iter().any(|p| p.in_mailbox);
    let carrier = residents
        .iter()
        .find(|r| r.job == Job::Post)
        .map(Resident::pose);
    if *round == Round::Approaching
        && (settings.reduced_motion
            || !pending
            || carrier
                .as_ref()
                .is_none_or(|p| p.returning || p.stop >= 0.54))
    {
        *round = Round::Posted;
    }
    if *round == Round::Posted
        && carrier
            .as_ref()
            .is_some_and(|p| p.returning && p.stop > 0.65)
    {
        *round = Round::Idle;
    }
    for (face, mut at, mut shown) in &mut envelopes {
        let street = *face == Face::Street;
        let here = if street {
            *location == Location::Market && *phase.get() == Phase::Day
        } else {
            *location == Location::Shop
        };
        shown.set_if_neq(if pending && here {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if !pending || !here {
            continue;
        }
        let slot = if street { STREET_SLOT } else { INNER_SLOT };
        let mut letter = Transform::from_translation(slot + TUCK)
            .with_rotation(Quat::from_rotation_x(-0.40) * Quat::from_rotation_z(0.10));
        if street
            && *round == Round::Approaching
            && let (Some(pose), Ok(gate)) = (&carrier, gate.single())
        {
            let transfer = ease((pose.stop - 0.18) / 0.34);
            let palm = pose
                .at
                .transform_point(hand(Vec3::new(0.29, 0.86, 0.183), pose.arm));
            let local = gate.affine().inverse().transform_point3(palm);
            letter.translation = local.lerp(letter.translation, transfer);
            let (_, rotation, _) = gate.to_scale_rotation_translation();
            letter.rotation =
                (rotation.inverse() * pose.at.rotation * Quat::from_rotation_x(pose.arm))
                    .slerp(letter.rotation, transfer);
            letter.scale = Vec3::splat(0.63 + 0.37 * transfer);
        }
        at.set_if_neq(letter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn fresh_mail_is_posted_on_the_round_and_waits_inside_when_the_market_is_skipped() {
        let data = crate::data::GameData::load();
        let post = data
            .props
            .iter()
            .find(|p| p.id == "letter_carrier")
            .unwrap();
        let route = post.walk.unwrap();
        let mut world = World::new();
        let mut progress = Progress::new(0);
        progress.night = 3;
        progress.journal.postcards.push(crate::postcards::Postcard {
            night: 2,
            in_mailbox: true,
            ..default()
        });
        world.insert_resource(progress);
        world.init_resource::<Settings>();
        world.init_resource::<Round>();
        world.insert_resource(State::new(Phase::Day));
        world.insert_resource(Location::Market);
        world.spawn((PostGate, GlobalTransform::default()));
        let carrier = world
            .spawn(Resident::new(post, route, super::super::world_pos(post)))
            .id();
        let street = world
            .spawn((Face::Street, Transform::default(), Visibility::Hidden))
            .id();
        let inside = world
            .spawn((Face::Shop, Transform::default(), Visibility::Hidden))
            .id();
        world.run_system_once(begin_round).unwrap();
        assert!(world.resource::<Round>() == &Round::Approaching);
        world.run_system_once(display).unwrap();
        assert_eq!(
            *world.get::<Visibility>(street).unwrap(),
            Visibility::Inherited
        );
        world.get_mut::<Resident>(carrier).unwrap().elapsed = route.travel + 0.65 * route.pause;
        world.run_system_once(display).unwrap();
        assert!(world.resource::<Round>() == &Round::Posted);
        assert_eq!(
            world.get::<Transform>(street).unwrap().translation,
            STREET_SLOT + TUCK
        );
        world.resource_mut::<Settings>().reduced_motion = true;
        world.run_system_once(begin_round).unwrap();
        assert!(world.resource::<Round>() == &Round::Posted);
        world.insert_resource(Round::Idle);
        world.insert_resource(State::new(Phase::Evening));
        world.insert_resource(Location::Shop);
        world.run_system_once(display).unwrap();
        assert_eq!(
            *world.get::<Visibility>(inside).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            world.get::<Transform>(inside).unwrap().translation,
            INNER_SLOT + TUCK
        );
        world.resource_mut::<Progress>().journal.postcards[0].in_mailbox = false;
        world.run_system_once(display).unwrap();
        assert_eq!(
            *world.get::<Visibility>(inside).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(world.resource::<Progress>().journal.postcards.len(), 1);
    }
}
