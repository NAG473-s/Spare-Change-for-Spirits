//! Playing with a controller (Steam Deck, Xbox, PlayStation …): the left
//! stick moves a pointer, A clicks and B right-clicks, the right stick
//! scrolls, the d-pad nudges the pointer, Start opens the pause menu.
//!
//! The pointer is the mouse cursor itself: the game moves it and sends the
//! same mouse messages a real mouse would, so everything that works with the
//! mouse – buttons, shelves, ghosts, crates, dragging furniture, tooltips –
//! works with the controller too. A ring marks the pointer while the
//! controller is in use; touching the real mouse hides it again.

use bevy::input::ButtonState;
use bevy::input::gamepad::GamepadButtonStateChangedEvent;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::picking::PickingSystems;
use bevy::picking::pointer::{PointerAction, PointerId, PointerInput, PointerLocation};
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow, WindowEvent};

/// Pointer speed at full tilt, in window heights per second.
const POINTER_SPEED: f32 = 0.9;
/// Stick input below this is ignored (worn sticks drift a little).
const DEAD_ZONE: f32 = 0.15;
/// Above 1 the pointer moves slowly for small tilts, for precise aiming.
const STICK_CURVE: f32 = 1.8;
/// The d-pad moves the pointer this many logical pixels a press, and
/// this fast (pixels a second) while held.
const NUDGE: f32 = 6.0;
const NUDGE_SPEED: f32 = 160.0;
/// Scrolling with the right stick at full tilt, in pixels a second.
const SCROLL_SPEED: f32 = 900.0;
const POINTER_BUTTONS: [(GamepadButton, MouseButton); 2] = [
    (GamepadButton::South, MouseButton::Left),
    (GamepadButton::East, MouseButton::Right),
];
/// Size of the ring that marks the pointer (logical pixels).
const RING: f32 = 26.0;

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PadPointer>()
            .add_systems(Startup, spawn_ring)
            .add_observer(disconnect_pointer)
            // Picking reads window events in First; regular mouse input follows
            // in PreUpdate. Both must see the same press in the same frame.
            .add_systems(
                First,
                (drive_pointer, forward_buttons)
                    .chain()
                    .after(bevy::time::TimeSystems)
                    .before(PickingSystems::Input),
            )
            .add_systems(Update, show_ring);
    }
}

/// Where the controller's pointer is and whether the controller is in use.
#[derive(Resource, Default)]
struct PadPointer {
    pos: Option<Vec2>,
    active: bool,
}

/// The ring that marks the pointer.
#[derive(Component)]
struct Ring;

/// Is any controller's Start button just pressed? (Opens the pause menu.)
pub fn start_pressed(gamepads: &Query<&Gamepad>) -> bool {
    gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::Start))
}

fn stick(gamepad: &Gamepad, x: GamepadAxis, y: GamepadAxis) -> Vec2 {
    let v = Vec2::new(
        gamepad.get(x).unwrap_or(0.0),
        // Up on the stick is up on the screen.
        -gamepad.get(y).unwrap_or(0.0),
    );
    let len = v.length();
    if len < DEAD_ZONE {
        return Vec2::ZERO;
    }
    let strength = ((len - DEAD_ZONE) / (1.0 - DEAD_ZONE)).min(1.0);
    v / len * strength.powf(STICK_CURVE)
}

fn drive_pointer(
    time: Res<Time<Real>>,
    gamepads: Query<&Gamepad>,
    mouse: Res<AccumulatedMouseMotion>,
    mut window: Single<(Entity, &mut Window), With<PrimaryWindow>>,
    mut pointer: ResMut<PadPointer>,
    mut window_events: MessageWriter<WindowEvent>,
    mut moved: MessageWriter<CursorMoved>,
    mut wheel: MessageWriter<MouseWheel>,
) {
    let (entity, window) = &mut *window;
    let entity = *entity;
    // The real mouse moved: it takes over again.
    if mouse.delta != Vec2::ZERO || gamepads.is_empty() {
        pointer.active = false;
        pointer.pos = None;
    }
    let dt = time.delta_secs();
    let size = window.size();
    for gamepad in &gamepads {
        let mut step = stick(gamepad, GamepadAxis::LeftStickX, GamepadAxis::LeftStickY)
            * POINTER_SPEED
            * size.y
            * dt;
        for (button, dir) in [
            (GamepadButton::DPadLeft, Vec2::NEG_X),
            (GamepadButton::DPadRight, Vec2::X),
            (GamepadButton::DPadUp, Vec2::NEG_Y),
            (GamepadButton::DPadDown, Vec2::Y),
        ] {
            if gamepad.just_pressed(button) {
                step += dir * NUDGE;
            } else if gamepad.pressed(button) {
                step += dir * NUDGE_SPEED * dt;
            }
        }
        let clicking = POINTER_BUTTONS
            .iter()
            .any(|&(b, _)| gamepad.just_pressed(b) || gamepad.just_released(b));
        let scroll = stick(gamepad, GamepadAxis::RightStickX, GamepadAxis::RightStickY);
        if step == Vec2::ZERO && !clicking && scroll == Vec2::ZERO {
            continue;
        }
        let from = pointer
            .pos
            .or_else(|| window.cursor_position())
            .unwrap_or(size / 2.0);
        let to = (from + step).clamp(Vec2::ZERO, (size - Vec2::ONE).max(Vec2::ZERO));
        if to != from || pointer.pos.is_none() {
            window.set_cursor_position(Some(to));
            let event = CursorMoved {
                window: entity,
                position: to,
                delta: Some(to - from),
            };
            moved.write(event.clone());
            window_events.write(WindowEvent::CursorMoved(event));
        }
        pointer.pos = Some(to);
        pointer.active = true;
        if scroll != Vec2::ZERO {
            // Pushing the stick down scrolls down, like the wheel turned
            // towards you.
            let event = MouseWheel {
                unit: MouseScrollUnit::Pixel,
                x: -scroll.x * SCROLL_SPEED * dt,
                y: -scroll.y * SCROLL_SPEED * dt,
                window: entity,
                phase: TouchPhase::Moved,
            };
            wheel.write(event);
            window_events.write(WindowEvent::MouseWheel(event));
        }
    }
}

/// Forward digital edges in arrival order, including several taps between frames.
fn forward_buttons(
    window: Single<Entity, With<PrimaryWindow>>,
    mut changes: MessageReader<GamepadButtonStateChangedEvent>,
    mut buttons: MessageWriter<MouseButtonInput>,
    mut window_events: MessageWriter<WindowEvent>,
) {
    for change in changes.read() {
        let Some((_, button)) = POINTER_BUTTONS
            .into_iter()
            .find(|(pad, _)| *pad == change.button)
        else {
            continue;
        };
        let event = MouseButtonInput {
            button,
            state: change.state,
            window: *window,
        };
        buttons.write(event);
        window_events.write(WindowEvent::MouseButtonInput(event));
    }
}

/// A disconnected pad cannot send its release. Cancel the interaction before
/// releasing the emulated mouse buttons, so unplugging never confirms a purchase.
fn disconnect_pointer(
    removed: On<Remove<Gamepad>>,
    gamepads: Query<&Gamepad>,
    window: Single<Entity, With<PrimaryWindow>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    mut inputs: MessageWriter<PointerInput>,
    mut buttons: MessageWriter<MouseButtonInput>,
    mut window_events: MessageWriter<WindowEvent>,
) {
    let Ok(pad) = gamepads.get(removed.entity) else {
        return;
    };
    if !POINTER_BUTTONS.iter().any(|(b, _)| pad.pressed(*b)) {
        return;
    }
    if let Some(location) = pointers
        .iter()
        .find(|(id, _)| **id == PointerId::Mouse)
        .and_then(|(_, pointer)| pointer.location())
    {
        inputs.write(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Cancel,
        ));
    }
    for (pad_button, button) in POINTER_BUTTONS {
        if pad.pressed(pad_button) {
            let event = MouseButtonInput {
                button,
                state: ButtonState::Released,
                window: *window,
            };
            buttons.write(event);
            window_events.write(WindowEvent::MouseButtonInput(event));
        }
    }
}

fn spawn_ring(mut commands: Commands) {
    commands.spawn((
        Ring,
        Node {
            position_type: PositionType::Absolute,
            width: px(RING),
            height: px(RING),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BorderColor::all(Color::srgba(1.0, 0.92, 0.7, 0.95)),
        BackgroundColor(Color::srgba(1.0, 0.92, 0.7, 0.15)),
        GlobalZIndex(1000),
        Visibility::Hidden,
        // Never in the way of what it points at.
        Pickable::IGNORE,
    ));
}

fn show_ring(
    pointer: Res<PadPointer>,
    ui_scale: Res<UiScale>,
    mut ring: Single<(&mut Node, &mut Visibility), With<Ring>>,
) {
    let (node, visibility) = &mut *ring;
    let shown = match pointer.pos.filter(|_| pointer.active) {
        Some(pos) => {
            // UI sizes are scaled by the interface size; the cursor is not.
            let at = pos / ui_scale.0 - Vec2::splat(RING / 2.0);
            node.left = px(at.x);
            node.top = px(at.y);
            Visibility::Inherited
        }
        None => Visibility::Hidden,
    };
    if **visibility != shown {
        **visibility = shown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_controller_taps_activate_the_button_and_release_the_mouse() {
        use bevy::app::HierarchyPropagatePlugin;
        use bevy::input::gamepad::{RawGamepadButtonChangedEvent, RawGamepadEvent};
        use bevy::ui::{ComputedUiTargetCamera, UiStack, update::propagate_ui_target_cameras};
        use bevy::ui_widgets::{Activate, Button, ButtonPlugin};

        #[derive(Component, Default)]
        struct Clicks(u32);

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::window::WindowPlugin::default(),
            bevy::input::InputPlugin,
            DefaultPickingPlugins,
            UiPickingPlugin,
            ButtonPlugin,
            HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate),
            GamepadPlugin,
        ))
        .init_resource::<UiScale>()
        .add_systems(Update, propagate_ui_target_cameras);

        let world = app.world_mut();
        let position = Vec2::new(80.0, 65.0);
        world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(world)
            .unwrap()
            .set_cursor_position(Some(position));
        let pad = world.spawn(Gamepad::default()).id();
        let camera = world.spawn(Camera2d).id();
        let button = world
            .spawn((
                Node::default(),
                Button,
                Clicks::default(),
                UiTargetCamera(camera),
                ComputedNode {
                    size: Vec2::splat(100.),
                    ..default()
                },
                UiGlobalTransform::from_translation(position),
                InheritedVisibility::VISIBLE,
            ))
            .observe(|event: On<Activate>, mut buttons: Query<&mut Clicks>| {
                buttons.get_mut(event.entity).unwrap().0 += 1;
            })
            .id();
        let mut stack = UiStack::default();
        stack.uinodes.push(button);
        stack.partition.push(0..1);
        world.insert_resource(stack);
        app.update();
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .write_message(WindowEvent::CursorMoved(CursorMoved {
                window,
                position,
                delta: None,
            }));
        app.update();

        // Two complete taps can arrive between frames, e.g. after a rendering stall.
        for value in [1., 0., 1., 0.] {
            app.world_mut().write_message(RawGamepadEvent::Button(
                RawGamepadButtonChangedEvent::new(pad, GamepadButton::South, value),
            ));
        }
        app.update();
        app.update();
        assert_eq!(app.world().get::<Clicks>(button).unwrap().0, 2);
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );

        // Losing the pad during the next press clears the drag without a third click.
        app.world_mut()
            .write_message(RawGamepadEvent::Button(RawGamepadButtonChangedEvent::new(
                pad,
                GamepadButton::South,
                1.,
            )));
        app.update();
        app.update();
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        use bevy::input::gamepad::{GamepadConnection, GamepadConnectionEvent};
        app.world_mut().write_message(GamepadConnectionEvent::new(
            pad,
            GamepadConnection::Disconnected,
        ));
        app.update();
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        assert_eq!(app.world().get::<Clicks>(button).unwrap().0, 2);
    }

    #[test]
    fn scrolling_can_be_the_first_controller_input() {
        let mut app = App::new();
        app.init_resource::<PadPointer>()
            .init_resource::<Time<Real>>()
            .init_resource::<AccumulatedMouseMotion>()
            .add_message::<WindowEvent>()
            .add_message::<CursorMoved>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseWheel>()
            .add_systems(Update, drive_pointer);
        let mut window = Window::default();
        let position = Vec2::new(75.0, 80.0);
        window.set_cursor_position(Some(position));
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::RightStickY, -1.0);
        app.world_mut().spawn(pad);
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(16));
        app.update();

        let pointer = app.world().resource::<PadPointer>();
        assert!(pointer.active);
        assert_eq!(pointer.pos, Some(position));
        let wheel = app.world().resource::<Messages<MouseWheel>>();
        let event = wheel.get_cursor().read(wheel).copied().next().unwrap();
        assert_eq!(event.window, entity);
        assert!(event.y < 0.0);
    }
}
