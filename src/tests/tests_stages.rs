use crate::action::Action;
use crate::config::{resolve_stages, Config};
use crate::event::{Event, KeyEvent, KeyValue};
use crate::tests::{assert_actions, EventHandlerForTest};
use evdev::KeyCode as Key;
use indoc::indoc;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_stage_output_is_remapped_by_next_stage() {
    assert_actions(
        indoc! {"
        stages:
          - - remap:
                A: B
          - - remap:
                B: C
        "},
        vec![Event::key_press(Key::KEY_A), Event::key_release(Key::KEY_A)],
        vec![
            Action::KeyEvent(KeyEvent::new(Key::KEY_C, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_C, KeyValue::Release)),
        ],
    );
}

#[test]
fn test_entries_in_one_stage_dont_see_each_others_output() {
    assert_actions(
        indoc! {"
        stages:
          - - remap:
                A: B
            - remap:
                B: C
        "},
        vec![Event::key_press(Key::KEY_A), Event::key_release(Key::KEY_A)],
        vec![
            Action::KeyEvent(KeyEvent::new(Key::KEY_B, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_B, KeyValue::Release)),
        ],
    );
}

#[test]
fn test_stage_output_goes_to_keymap() {
    assert_actions(
        indoc! {"
        stages:
          - - remap:
                A: B
        keymap:
          - remap:
              B: C
        "},
        vec![Event::key_press(Key::KEY_A), Event::key_release(Key::KEY_A)],
        vec![
            Action::KeyEvent(KeyEvent::new(Key::KEY_C, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_C, KeyValue::Release)),
            Action::Delay(Duration::from_nanos(0)),
            Action::Delay(Duration::from_nanos(0)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_B, KeyValue::Release)),
        ],
    );
}

#[test]
fn test_tick_reaches_later_stage() {
    let mut handler = EventHandlerForTest::new(indoc! {"
        stages:
          - - remap:
                A: B
          - - remap:
                X:
                  tap_hold_next_release:
                    tap: X
                    hold: LEFTSHIFT
                    timeout: 10
        "});

    handler.assert(vec![Event::key_press(Key::KEY_X)], vec![]);
    sleep(Duration::from_millis(20));
    handler.assert(vec![Event::Tick], vec![Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Press))]);
    handler.assert(
        vec![Event::key_release(Key::KEY_X)],
        vec![Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Release))],
    );
}

#[test]
fn test_mode_set_in_stage_applies_to_next_stage() {
    // The layer key decides hold on A's release, and then replays A's press and release.
    // The next stage must see them in the new mode.
    assert_actions(
        indoc! {"
        stages:
          - - remap:
                X:
                  tap_hold_next_release:
                    tap: X
                    hold: { set_mode: nav }
          - - mode: nav
              remap:
                A: LEFT
        "},
        vec![
            Event::key_press(Key::KEY_X),
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
            Event::key_release(Key::KEY_X),
            Event::key_press(Key::KEY_A),
        ],
        vec![
            Action::KeyEvent(KeyEvent::new(Key::KEY_LEFT, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_LEFT, KeyValue::Release)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_A, KeyValue::Press)),
        ],
    );
}

#[test]
fn test_legacy_sections_become_experimental_then_modmap_stage() {
    // experimental_map runs first, so its output is remapped by modmap.
    assert_actions(
        indoc! {"
        experimental_map:
          - remap:
              X:
                tap_hold_next_release:
                  tap: A
                  hold: LEFTSHIFT
        modmap:
          - remap:
              A: B
        "},
        vec![Event::key_press(Key::KEY_X), Event::key_release(Key::KEY_X)],
        vec![
            Action::KeyEvent(KeyEvent::new(Key::KEY_B, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_B, KeyValue::Release)),
        ],
    );
}

#[test]
fn test_stages_cannot_be_combined_with_legacy_sections() {
    let mut config: Config = serde_yaml::from_str(indoc! {"
        stages:
          - - remap:
                A: B
        modmap:
          - remap:
              C: D
        "})
    .unwrap();

    let err = resolve_stages(&mut config).unwrap_err().to_string();
    assert_eq!(err, "`stages` can't be combined with `experimental_map` or `modmap`");
}

#[test]
fn test_multipurpose_key_is_interrupted_by_another_multipurpose_key() {
    // The second multi-purpose key is a pressed key like any other, so it interrupts the first.
    assert_actions(
        indoc! {"
        stages:
          - - remap:
                F:
                  held: LEFTSHIFT
                  alone: F
                D:
                  held: LEFTCTRL
                  alone: D
        "},
        vec![
            Event::key_press(Key::KEY_F),
            Event::key_press(Key::KEY_D),
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
            Event::key_release(Key::KEY_D),
            Event::key_release(Key::KEY_F),
        ],
        vec![
            Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTCTRL, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_A, KeyValue::Press)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_A, KeyValue::Release)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTCTRL, KeyValue::Release)),
            Action::KeyEvent(KeyEvent::new(Key::KEY_LEFTSHIFT, KeyValue::Release)),
        ],
    );
}

#[test]
fn test_press_release_key_in_stage() {
    assert_actions(
        indoc! {"
        stages:
          - - remap:
                A:
                  press: { set_mode: other }
                  skip_key_event: true
          - - mode: other
              remap:
                B: C
        "},
        vec![
            Event::key_press(Key::KEY_A),
            Event::key_release(Key::KEY_A),
            Event::key_press(Key::KEY_B),
        ],
        vec![Action::KeyEvent(KeyEvent::new(Key::KEY_C, KeyValue::Press))],
    );
}
