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
use bevy::input::mouse::{AccumulatedMouseMotion, MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::picking::PickingSystems;
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
/// Size of the ring that marks the pointer (logical pixels).
const RING: f32 = 26.0;

pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PadPointer>()
            .add_systems(Startup, spawn_ring)
            // Picking reads window events in First; regular mouse input follows
            // in PreUpdate. Both must see the same press in the same frame.
            .add_systems(
                First,
                drive_pointer
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
    mut buttons: MessageWriter<MouseButtonInput>,
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
        let clicks = [
            (GamepadButton::South, MouseButton::Left),
            (GamepadButton::East, MouseButton::Right),
        ];
        let clicking = clicks
            .iter()
            .any(|(b, _)| gamepad.just_pressed(*b) || gamepad.just_released(*b));
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
            pointer.active = true;
        }
        pointer.pos = Some(to);
        for (pad, button) in clicks {
            let state = if gamepad.just_pressed(pad) {
                ButtonState::Pressed
            } else if gamepad.just_released(pad) {
                ButtonState::Released
            } else {
                continue;
            };
            let event = MouseButtonInput {
                button,
                state,
                window: entity,
            };
            buttons.write(event);
            window_events.write(WindowEvent::MouseButtonInput(event));
            pointer.active = true;
        }
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
