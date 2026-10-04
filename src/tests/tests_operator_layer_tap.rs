use crate::action::Action;
use crate::event::{Event, KeyEvent, KeyValue};
use crate::tests::EventHandlerForTest;
use evdev::KeyCode as Key;
use indoc::indoc;
use std::thread;
use std::time::Duration;

static TIMEOUT: Duration = Duration::from_millis(20);

// KP2 is a layer-tap: tap -> A, hold -> numbers mode, where KP9 is 5 instead of E.
fn get_handler(device: &str) -> EventHandlerForTest {
    EventHandlerForTest::new(&format!(
        indoc! {"
        experimental_map:
          - device:
              only: ['{}']
            remap:
              KP2:
                tap_hold_next_release:
                  tap: KP2
                  hold: {{ set_mode: numbers }}
                  timeout: 20
        modmap:
          - mode: [numbers]
            remap:
              KP9: KEY_5
          - remap:
              KP2: A
              KP9: E
        "},
        device
    ))
}

fn press(key: Key) -> Action {
    Action::KeyEvent(KeyEvent::new(key, KeyValue::Press))
}

fn release(key: Key) -> Action {
    Action::KeyEvent(KeyEvent::new(key, KeyValue::Release))
}

#[test]
fn test_layer_tap_tap() {
    let mut handler = get_handler("Some Device");

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![press(Key::KEY_A), release(Key::KEY_A)]);
}

#[test]
fn test_layer_tap_hold_by_next_release() {
    let mut handler = get_handler("Some Device");

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    // KP9 released while KP2 is held -> hold, and the buffered KP9 sees numbers mode.
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![press(Key::KEY_5), release(Key::KEY_5)]);
    // Still in numbers mode while held.
    handler.assert(
        vec![Event::key_press(Key::KEY_KP9), Event::key_release(Key::KEY_KP9)],
        vec![press(Key::KEY_5), release(Key::KEY_5)],
    );
    // Releasing KP2 resets the mode, and emits nothing.
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![]);
    handler.assert(
        vec![Event::key_press(Key::KEY_KP9), Event::key_release(Key::KEY_KP9)],
        vec![press(Key::KEY_E), release(Key::KEY_E)],
    );
}

#[test]
fn test_layer_tap_roll_is_tap() {
    let mut handler = get_handler("Some Device");

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    // KP2 released first -> tap, and KP9 stays in default mode.
    handler.assert(
        vec![Event::key_release(Key::KEY_KP2)],
        vec![press(Key::KEY_A), release(Key::KEY_A), press(Key::KEY_E)],
    );
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_E)]);
}

#[test]
fn test_layer_tap_hold_by_timeout() {
    let mut handler = get_handler("Some Device");

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    thread::sleep(TIMEOUT);
    handler.assert(vec![Event::Tick], vec![]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![press(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![]);
}

#[test]
fn test_layer_tap_key_held_across_mode_reset() {
    let mut handler = get_handler("Some Device");

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    thread::sleep(TIMEOUT);
    handler.assert(vec![Event::Tick], vec![]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![press(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![]);
    // Released after the mode was reset, but still releases what was pressed.
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_5)]);
}

#[test]
fn test_layer_tap_other_device() {
    let mut handler = get_handler("Other Device");

    // Not the configured device, so KP2 is a plain key.
    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![press(Key::KEY_A)]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![press(Key::KEY_E)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_E)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![release(Key::KEY_A)]);
}

fn other_device() -> std::rc::Rc<crate::device::InputDeviceInfo> {
    std::rc::Rc::new(crate::device::InputDeviceInfo {
        name: "Other Device".into(),
        path: std::path::PathBuf::from("/dev/input/event1"),
        vendor: 0x1234,
        product: 0x5678,
    })
}

#[test]
fn test_layer_tap_same_key_on_other_device() {
    let mut handler = EventHandlerForTest::new(indoc! {"
        experimental_map:
          - device:
              only: ['Some Device']
            remap:
              KP2:
                tap_hold_next_release:
                  tap: KP2
                  hold: { set_mode: numbers }
                  timeout: 20
        modmap:
          - mode: [numbers]
            device:
              only: ['Other Device']
            remap:
              KP2: EQUAL
          - remap:
              KP2: A
        "});
    let other = other_device();

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    handler.assert(
        vec![Event::KeyEvent(
            other.clone(),
            KeyEvent::new(Key::KEY_KP2, KeyValue::Press),
        )],
        vec![],
    );
    // The other device's KP2 is not the trigger, so this decides hold.
    handler.assert(
        vec![Event::KeyEvent(
            other.clone(),
            KeyEvent::new(Key::KEY_KP2, KeyValue::Release),
        )],
        vec![press(Key::KEY_EQUAL), release(Key::KEY_EQUAL)],
    );
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![]);
}

// KP9 is a homerow mod (tap -> E, hold -> RIGHTSHIFT) only in default mode.
// In numbers mode it is a plain 5, set either by the KP2 layer-tap or by KPDOT.
fn get_mode_filter_handler() -> EventHandlerForTest {
    EventHandlerForTest::new(indoc! {"
        experimental_map:
          - remap:
              KP2:
                tap_hold_next_release:
                  tap: KP2
                  hold: { set_mode: numbers }
                  timeout: 20
          - mode: default
            remap:
              KP9:
                tap_hold_next_release:
                  tap: KP9
                  hold: RIGHTSHIFT
                  timeout: 20
        modmap:
          - mode: [numbers]
            remap:
              KP9: KEY_5
          - remap:
              KP2: A
              KP9: E
              KPDOT:
                skip_key_event: true
                press: { set_mode: numbers }
                release: { set_mode: default }
        "})
}

#[test]
fn test_mode_filter_default_mode() {
    let mut handler = get_mode_filter_handler();

    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![press(Key::KEY_E), release(Key::KEY_E)]);
}

#[test]
fn test_mode_filter_mode_from_modmap() {
    let mut handler = get_mode_filter_handler();

    handler.assert(vec![Event::key_press(Key::KEY_KPDOT)], vec![]);
    // No tap-hold in numbers mode, so 5 is pressed right away.
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![press(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KPDOT)], vec![]);

    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![press(Key::KEY_E), release(Key::KEY_E)]);
}

#[test]
fn test_mode_filter_replayed_after_layer_tap() {
    let mut handler = get_mode_filter_handler();

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    // KP9 was pressed in default mode, but is replayed after the hold set numbers mode,
    // so it is a plain 5 and not a tap-hold.
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![press(Key::KEY_5), release(Key::KEY_5)]);
    // Held in numbers mode, KP9 is pressed right away instead of waiting for a decision.
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![press(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![]);
}

#[test]
fn test_mode_filter_held_mod_survives_mode_change() {
    let mut handler = get_mode_filter_handler();

    // Shift is decided in default mode, and stays held while the mode changes.
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    thread::sleep(TIMEOUT);
    handler.assert(vec![Event::Tick], vec![press(Key::KEY_RIGHTSHIFT)]);
    handler.assert(vec![Event::key_press(Key::KEY_KPDOT)], vec![]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_RIGHTSHIFT)]);
    handler.assert(vec![Event::key_release(Key::KEY_KPDOT)], vec![]);
}

#[test]
fn test_mode_filter_replayed_while_still_held() {
    let mut handler = get_mode_filter_handler();

    handler.assert(vec![Event::key_press(Key::KEY_KP2)], vec![]);
    handler.assert(vec![Event::key_press(Key::KEY_KP9)], vec![]);
    thread::sleep(TIMEOUT);
    // The timeout decides hold. KP9 is replayed in numbers mode, so 5 is pressed
    // right away instead of KP9 starting a tap-hold.
    handler.assert(vec![Event::Tick], vec![press(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP9)], vec![release(Key::KEY_5)]);
    handler.assert(vec![Event::key_release(Key::KEY_KP2)], vec![]);
}
