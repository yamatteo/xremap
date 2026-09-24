use crate::event::{Event, KeyEvent};
use crate::operator_handler::OperatorHandler;
use crate::tests::{assert_events, get_handler_from_config};
use evdev::KeyCode as Key;
use indoc::indoc;
use std::thread;
use std::time::Duration;

static TIMEOUT: Duration = Duration::from_millis(20);

fn get_handler() -> OperatorHandler {
    get_handler_from_config(indoc! {"
        experimental_map:
          - remap:
              capslock:
                tap_hold_next_release:
                  tap: esc
                  hold: leftctrl
                  timeout: 20
        "})
    .unwrap()
}

#[test]
fn test_tap_hold_next_release_alone_tap() {
    let mut handler = get_handler();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
        ],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_sequential_tap() {
    let mut handler = get_handler();

    // Tap Capslock -> Esc
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
        ],
    );

    // Tap A -> A
    assert_events(
        handler.map_evs(vec![Event::key_press(Key::KEY_A)]),
        vec![Event::key_press(Key::KEY_A)],
    );
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![Event::key_release(Key::KEY_A)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_roll_released_first() {
    // Typing roll: Pesc Pa Resc Ra -> xa
    let mut handler = get_handler();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_A)]), vec![]);

    // Capslock released before A was released -> resolves to TAP!
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
            Event::key_press(Key::KEY_A),
        ],
    );

    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![Event::key_release(Key::KEY_A)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_pre_pressed_release() {
    // Pa Pesc Ra Resc -> ax
    let mut handler = get_handler();

    assert_events(
        handler.map_evs(vec![Event::key_press(Key::KEY_A)]),
        vec![Event::key_press(Key::KEY_A)],
    );

    // Press Capslock while A is already down
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);

    // Release A. Since A was pressed BEFORE Capslock, it does NOT trigger hold!
    assert_events(handler.map_evs(vec![Event::key_release(Key::KEY_A)]), vec![]);

    // Release Capslock -> TAP!
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
            Event::key_release(Key::KEY_A),
        ],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_modifier_hold() {
    // Pesc Ta Resc -> A (shifted/ctrl'd)
    let mut handler = get_handler();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_A)]), vec![]);

    // Release A while Capslock is held -> triggers HOLD!
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![
            Event::key_press(Key::KEY_LEFTCTRL),
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
        ],
    );

    // Release Capslock -> releases LeftCtrl
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![Event::key_release(Key::KEY_LEFTCTRL)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_timeout_without_other_keys() {
    let mut handler = get_handler();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::Tick]), vec![]);

    thread::sleep(TIMEOUT);

    // Timeout elapsed -> switches to HOLD!
    assert_events(
        handler.map_evs(vec![Event::Tick]),
        vec![Event::key_press(Key::KEY_LEFTCTRL)],
    );

    // Subsequent press passes through while held
    assert_events(
        handler.map_evs(vec![Event::key_press(Key::KEY_A)]),
        vec![Event::key_press(Key::KEY_A)],
    );
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![Event::key_release(Key::KEY_A)],
    );

    // Release Capslock -> releases LeftCtrl
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![Event::key_release(Key::KEY_LEFTCTRL)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_timeout_with_buffered_key() {
    let mut handler = get_handler();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_A)]), vec![]);

    thread::sleep(TIMEOUT);

    // Timeout elapsed while A is buffered -> emits LeftCtrl press then A press
    assert_events(
        handler.map_evs(vec![Event::Tick]),
        vec![
            Event::key_press(Key::KEY_LEFTCTRL),
            Event::key_press(Key::KEY_A),
        ],
    );

    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![Event::key_release(Key::KEY_A)],
    );

    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![Event::key_release(Key::KEY_LEFTCTRL)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_timeout_button() {
    let mut handler = get_handler_from_config(indoc! {"
        experimental_map:
          - remap:
              capslock:
                tap_hold_next_release:
                  tap: x
                  hold: leftshift
                  timeout: 20
                  timeout_button: x
        "})
    .unwrap();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);

    thread::sleep(TIMEOUT);

    // Timeout elapsed -> emits timeout_button (x) instead of hold (leftshift)
    assert_events(
        handler.map_evs(vec![Event::Tick]),
        vec![Event::key_press(Key::KEY_X)],
    );

    // Repeat while held repeats the timeout_button
    assert_events(
        handler.map_evs(vec![Event::key_repeat(Key::KEY_CAPSLOCK)]),
        vec![Event::key_repeat(Key::KEY_X)],
    );

    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![Event::key_release(Key::KEY_X)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_kebab_case() {
    let mut handler = get_handler_from_config(indoc! {"
        experimental_map:
          - remap:
              capslock:
                tap-hold-next-release:
                  tap: esc
                  hold: leftctrl
                  timeout: 20
        "})
    .unwrap();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
        ],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_multiple_keys() {
    let mut handler = get_handler_from_config(indoc! {"
        experimental_map:
          - remap:
              capslock:
                tap_hold_next_release:
                  tap: [leftctrl, c]
                  hold: [leftshift, leftalt]
                  timeout: 20
        "})
    .unwrap();

    // Tap emits sequential press and reverse release
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_LEFTCTRL),
            Event::key_press(Key::KEY_C),
            Event::key_release(Key::KEY_C),
            Event::key_release(Key::KEY_LEFTCTRL),
        ],
    );

    // Hold emits both modifiers, releases in reverse
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_A)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![
            Event::key_press(Key::KEY_LEFTSHIFT),
            Event::key_press(Key::KEY_LEFTALT),
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
        ],
    );
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_release(Key::KEY_LEFTALT),
            Event::key_release(Key::KEY_LEFTSHIFT),
        ],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_repeats() {
    let mut handler = get_handler();

    // Repeat while undecided is suppressed
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::key_repeat(Key::KEY_CAPSLOCK)]), vec![]);

    // Release -> Esc tap
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
        ],
    );

    // Repeat while held
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_A)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_A)]),
        vec![
            Event::key_press(Key::KEY_LEFTCTRL),
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
        ],
    );
    assert_events(
        handler.map_evs(vec![Event::key_repeat(Key::KEY_CAPSLOCK)]),
        vec![Event::key_repeat(Key::KEY_LEFTCTRL)],
    );
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![Event::key_release(Key::KEY_LEFTCTRL)],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_select() {
    let mut handler = get_handler_from_config(indoc! {"
        experimental_map:
          - remap:
              capslock:
                select:
                  - tap_hold_next_release:
                      tap: esc
                      hold: leftctrl
                      timeout: 20
        "})
    .unwrap();

    assert_events(handler.map_evs(vec![Event::key_press(Key::KEY_CAPSLOCK)]), vec![]);
    assert_events(
        handler.map_evs(vec![Event::key_release(Key::KEY_CAPSLOCK)]),
        vec![
            Event::key_press(Key::KEY_ESC),
            Event::key_release(Key::KEY_ESC),
        ],
    );

    handler.assert_base_state();
}

#[test]
fn test_tap_hold_next_release_integration_actions() {
    crate::tests::assert_actions(
        indoc! {"
        experimental_map:
          - remap:
              capslock:
                tap_hold_next_release:
                  tap: esc
                  hold: leftshift
                  timeout: 20
        "},
        vec![
            Event::key_press(Key::KEY_CAPSLOCK),
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
            Event::key_release(Key::KEY_CAPSLOCK),
        ],
        vec![
            crate::action::Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTSHIFT, crate::event::KeyValue::Press)),
            crate::action::Action::KeyEvent(KeyEvent::new(Key::KEY_A, crate::event::KeyValue::Press)),
            crate::action::Action::KeyEvent(KeyEvent::new(Key::KEY_A, crate::event::KeyValue::Release)),
            crate::action::Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTSHIFT, crate::event::KeyValue::Release)),
        ],
    );
}

