//! The modmap remappings as operators: plain keys, multi-purpose keys and press/release keys.
//!
//! Each becomes active when its trigger key is pressed, and stays active until that key
//! is released. So repeat and release always match the press, even if the mode or the
//! window changes in between.
use crate::config::keymap_action::KeymapAction;
use crate::config::modmap_operator::{Interruptable, Keys, MultiPurposeKey, PressReleaseKey};
use crate::device::InputDeviceInfo;
use crate::emit_handler::Emit;
use crate::event::{Event, KeyEvent};
use crate::event_handler::{MODIFIER_KEYS, PRESS, RELEASE, REPEAT};
use crate::operators::{ActiveOperator, OperatorAction, StaticOperator};
use evdev::KeyCode as Key;
use log::warn;
use std::cmp::Ordering;
use std::rc::Rc;
use std::time::{Duration, Instant};

// The same keycode on another device is a different key.
fn is_trigger(trigger: &(Rc<InputDeviceInfo>, Key), device: &InputDeviceInfo, key: Key) -> bool {
    key == trigger.1 && device.path == trigger.0.path
}

fn trigger_of(event: &Event) -> (Rc<InputDeviceInfo>, Key) {
    match event {
        Event::KeyEvent(device, key_event) => (device.clone(), key_event.key),
        _ => unreachable!(),
    }
}

fn emit_keys(device: &Rc<InputDeviceInfo>, keys: Vec<(Key, i32)>) -> Vec<Emit> {
    keys.into_iter()
        .map(|(key, value)| Emit::key_event(device.clone(), KeyEvent::new_with(key.code(), value)))
        .collect()
}

// --- Keys

#[derive(Debug)]
pub struct KeysOperator {
    keys: Vec<Key>,
}

impl KeysOperator {
    pub fn get_ops(key: Key, keys: &Keys) -> Vec<(Key, Box<dyn StaticOperator>)> {
        vec![(
            key,
            Box::new(KeysOperator {
                keys: keys.clone().into_vec(),
            }),
        )]
    }
}

impl StaticOperator for KeysOperator {
    fn get_active_operator(&self, event: &Event) -> Box<dyn ActiveOperator> {
        Box::new(ActiveKeysOperator {
            trigger: trigger_of(event),
            keys: self.keys.clone(),
        })
    }
}

#[derive(Debug)]
struct ActiveKeysOperator {
    trigger: (Rc<InputDeviceInfo>, Key),
    keys: Vec<Key>,
}

impl ActiveKeysOperator {
    fn emit(&self, device: &Rc<InputDeviceInfo>, value: i32) -> Vec<Emit> {
        emit_keys(device, self.keys.iter().map(|key| (*key, value)).collect())
    }
}

impl ActiveOperator for ActiveKeysOperator {
    fn on_event(&mut self, event: &Event) -> OperatorAction {
        match event {
            Event::KeyEvent(device, key_event) if is_trigger(&self.trigger, device, key_event.key) => {
                let value = key_event.value();
                if value == RELEASE {
                    OperatorAction::Done(self.emit(device, value), vec![])
                } else {
                    OperatorAction::Partial(self.emit(device, value), vec![])
                }
            }
            _ => OperatorAction::Unhandled,
        }
    }
}

// --- Press/release keys

#[derive(Debug)]
pub struct PressReleaseOperator {
    config: PressReleaseKey,
}

impl PressReleaseOperator {
    pub fn get_ops(key: Key, config: &PressReleaseKey) -> Vec<(Key, Box<dyn StaticOperator>)> {
        vec![(key, Box::new(PressReleaseOperator { config: config.clone() }))]
    }
}

impl StaticOperator for PressReleaseOperator {
    fn get_active_operator(&self, event: &Event) -> Box<dyn ActiveOperator> {
        Box::new(ActivePressReleaseOperator {
            trigger: trigger_of(event),
            config: self.config.clone(),
        })
    }
}

#[derive(Debug)]
struct ActivePressReleaseOperator {
    trigger: (Rc<InputDeviceInfo>, Key),
    config: PressReleaseKey,
}

impl ActiveOperator for ActivePressReleaseOperator {
    fn on_event(&mut self, event: &Event) -> OperatorAction {
        let (device, key_event) = match event {
            Event::KeyEvent(device, key_event) if is_trigger(&self.trigger, device, key_event.key) => {
                (device, key_event)
            }
            _ => return OperatorAction::Unhandled,
        };

        let actions: &Vec<KeymapAction> = match key_event.value() {
            PRESS => &self.config.press,
            RELEASE => &self.config.release,
            _ => &self.config.repeat,
        };

        // The actions are run by keymap, before the original key reaches it.
        let mut emit = vec![];
        if !actions.is_empty() {
            emit.push(Emit::Single(Event::KeymapActions(key_event.key, actions.clone())));
        }
        if !self.config.skip_key_event {
            emit.push(Emit::key_event(device.clone(), key_event.clone()));
        }

        if key_event.value() == RELEASE {
            OperatorAction::Done(emit, vec![])
        } else {
            OperatorAction::Partial(emit, vec![])
        }
    }
}

// --- Multi-purpose keys

#[derive(Debug)]
pub struct MultiPurposeOperator {
    config: MultiPurposeKey,
}

impl MultiPurposeOperator {
    pub fn get_ops(key: Key, config: &MultiPurposeKey) -> Vec<(Key, Box<dyn StaticOperator>)> {
        vec![(key, Box::new(MultiPurposeOperator { config: config.clone() }))]
    }
}

impl StaticOperator for MultiPurposeOperator {
    fn get_active_operator(&self, event: &Event) -> Box<dyn ActiveOperator> {
        let MultiPurposeKey {
            hold,
            tap,
            hold_threshold,
            tap_timeout,
            free_hold,
            interruptable,
        } = self.config.clone();

        let hold_threshold = if hold_threshold <= tap_timeout {
            hold_threshold
        } else {
            warn!("hold_threshold_millis must be smaller than tap_timeout_millis. Setting hold_threshold_millis to tap_timeout_millis: {:?}", tap_timeout);
            tap_timeout
        };

        let now = Instant::now();
        Box::new(ActiveMultiPurposeOperator {
            trigger: trigger_of(event),
            state: MultiPurposeKeyState {
                hold,
                tap,
                interruptable,
                hold_threshold_at: now + hold_threshold,
                tap_timeout_at: if free_hold {
                    // An approximation of never.
                    now + Duration::from_secs_f32(1e10)
                } else {
                    now + tap_timeout
                },
                state: if hold_threshold == Duration::ZERO {
                    MultiPurposeKeyStateEnum::HoldPreferred
                } else {
                    MultiPurposeKeyStateEnum::TapPreferred
                },
            },
            pressed: false,
        })
    }
}

#[derive(Debug)]
struct ActiveMultiPurposeOperator {
    trigger: (Rc<InputDeviceInfo>, Key),
    state: MultiPurposeKeyState,
    // The press of the trigger has been seen.
    pressed: bool,
}

impl ActiveMultiPurposeOperator {
    // Another key was pressed (or the mouse used): emit tap or hold first, if it interrupts,
    // and let the interrupting event go on. It comes back to this operator, which now
    // returns it as unhandled.
    fn interrupt(&mut self, device: &Rc<InputDeviceInfo>, key: Key, event: &Event) -> OperatorAction {
        let flushed = self.state.interrupted_by_press(&[key]);
        if flushed.is_empty() {
            return OperatorAction::Unhandled;
        }

        // When the interrupting key is one the tap or hold just pressed, it's not pressed again.
        let already_pressed = flushed.iter().any(|(k, v)| *k == key && *v == PRESS);
        let unhandled = match event {
            Event::KeyEvent(_, key_event) if already_pressed && key_event.value() == PRESS => vec![],
            _ => vec![event.clone()],
        };
        OperatorAction::Partial(emit_keys(device, flushed), unhandled)
    }
}

impl ActiveOperator for ActiveMultiPurposeOperator {
    fn on_event(&mut self, event: &Event) -> OperatorAction {
        match event {
            Event::KeyEvent(device, key_event) if is_trigger(&self.trigger, device, key_event.key) => {
                match key_event.value() {
                    PRESS if !self.pressed => {
                        // Delay the press
                        self.pressed = true;
                        OperatorAction::Partial(vec![], vec![])
                    }
                    PRESS | REPEAT => OperatorAction::Partial(emit_keys(device, self.state.repeat()), vec![]),
                    _ => OperatorAction::Done(emit_keys(device, self.state.release()), vec![]),
                }
            }
            Event::KeyEvent(device, key_event) if key_event.value() == PRESS => {
                self.interrupt(device, key_event.key, event)
            }
            Event::RelativeEvent(device, relative_event) => {
                self.interrupt(device, relative_event.to_disguised_key(), event)
            }
            _ => OperatorAction::Unhandled,
        }
    }
}

#[derive(Debug, PartialEq)]
enum MultiPurposeKeyStateEnum {
    // If released then the tab-action is emitted
    // If interrupted then the tab-action is emitted
    // If timeout go to HoldPreferred state
    TapPreferred,
    // If released then the tab-action is emitted
    // If interrupted then the hold-action is emitted
    // If timeout then hold-action is emitted
    HoldPreferred,
    // Tab-action has been pressed and released.
    TapChosen,
    // Hold-action has been pressed, but has not been released yet.
    HoldDown,
}

#[derive(Debug)]
struct MultiPurposeKeyState {
    hold: Keys,
    tap: Keys,
    interruptable: Interruptable,
    hold_threshold_at: Instant,
    tap_timeout_at: Instant,
    state: MultiPurposeKeyStateEnum,
}

impl MultiPurposeKeyState {
    /// It causes a problem, that repeat is used for timeout. Partially because its interval
    /// gives low precision. And because the repeat events will stop if another key is pressed. Also
    /// BTN_RIGHT does not emit repeat events, so mouse buttons can't be remapped this way.
    fn repeat(&mut self) -> Vec<(Key, i32)> {
        if matches!(self.state, MultiPurposeKeyStateEnum::TapPreferred) && Instant::now() >= self.hold_threshold_at {
            // Timeout. Setting state before going into the switch is necessary
            self.state = MultiPurposeKeyStateEnum::HoldPreferred;
        }

        match self.state {
            MultiPurposeKeyStateEnum::TapPreferred => {
                vec![] // still delay repeat
            }
            MultiPurposeKeyStateEnum::HoldPreferred if Instant::now() < self.tap_timeout_at => {
                vec![] // still delay repeat
            }
            MultiPurposeKeyStateEnum::HoldPreferred => {
                // timeout
                self.state = MultiPurposeKeyStateEnum::HoldDown;
                let mut keys = self.hold.clone().into_vec();
                keys.sort_by(modifiers_first);
                keys.into_iter().map(|key| (key, PRESS)).collect()
            }
            MultiPurposeKeyStateEnum::HoldDown => {
                let mut keys = self.hold.clone().into_vec();
                keys.sort_by(modifiers_first);
                keys.into_iter().map(|key| (key, REPEAT)).collect()
            }
            MultiPurposeKeyStateEnum::TapChosen => {
                vec![] // tap-action already released, so ignores repeat
            }
        }
    }

    fn release(&mut self) -> Vec<(Key, i32)> {
        if matches!(self.state, MultiPurposeKeyStateEnum::TapPreferred) && Instant::now() >= self.hold_threshold_at {
            // Timeout. Setting state before going into the switch is necessary
            self.state = MultiPurposeKeyStateEnum::HoldPreferred;
        }

        match self.state {
            MultiPurposeKeyStateEnum::TapPreferred => {
                // Before hold_threshold_at timeout
                self.press_and_release(&self.tap)
            }
            MultiPurposeKeyStateEnum::HoldPreferred if Instant::now() < self.tap_timeout_at => {
                self.press_and_release(&self.tap)
            }
            MultiPurposeKeyStateEnum::HoldPreferred => self.press_and_release(&self.hold),
            MultiPurposeKeyStateEnum::HoldDown => {
                let mut release_keys = self.hold.clone().into_vec();
                release_keys.sort_by(modifiers_last);
                release_keys.into_iter().map(|key| (key, RELEASE)).collect()
            }
            MultiPurposeKeyStateEnum::TapChosen => {
                vec![] // nothing to release
            }
        }
    }

    // Other keys were pressed, so the multipurpose key
    // should emit presses of its held-value if it can be interrupted by those keys.
    fn interrupted_by_press(&mut self, pressed: &[Key]) -> Vec<(Key, i32)> {
        if !pressed.iter().any(|key| self.interruptable.is_interrupted_by(*key)) {
            return vec![];
        }

        if matches!(self.state, MultiPurposeKeyStateEnum::TapPreferred) && Instant::now() >= self.hold_threshold_at {
            // Timeout. Setting state before going into the switch is necessary
            self.state = MultiPurposeKeyStateEnum::HoldPreferred;
        }

        match self.state {
            MultiPurposeKeyStateEnum::TapPreferred => {
                self.state = MultiPurposeKeyStateEnum::TapChosen;
                self.press_and_release(&self.tap)
            }
            MultiPurposeKeyStateEnum::HoldPreferred => {
                self.state = MultiPurposeKeyStateEnum::HoldDown;

                let mut keys = self.hold.clone().into_vec();
                keys.sort_by(modifiers_first);
                keys.into_iter().map(|key| (key, PRESS)).collect()
            }
            MultiPurposeKeyStateEnum::HoldDown | MultiPurposeKeyStateEnum::TapChosen => vec![],
        }
    }

    fn press_and_release(&self, keys_to_use: &Keys) -> Vec<(Key, i32)> {
        let mut release_keys = keys_to_use.clone().into_vec();
        release_keys.sort_by(modifiers_last);
        let release_events: Vec<(Key, i32)> = release_keys.into_iter().map(|key| (key, RELEASE)).collect();

        let mut press_keys = keys_to_use.clone().into_vec();
        press_keys.sort_by(modifiers_first);
        let mut events: Vec<(Key, i32)> = press_keys.into_iter().map(|key| (key, PRESS)).collect();
        events.extend(release_events);
        events
    }
}

/// Orders modifier keys ahead of non-modifier keys.
/// Unfortunately the underlying type doesn't allow direct
/// comparison, but that's ok for our purposes.
fn modifiers_first(a: &Key, b: &Key) -> Ordering {
    if MODIFIER_KEYS.contains(a) {
        if MODIFIER_KEYS.contains(b) {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    } else if MODIFIER_KEYS.contains(b) {
        Ordering::Greater
    } else {
        // Neither are modifiers
        Ordering::Equal
    }
}

fn modifiers_last(a: &Key, b: &Key) -> Ordering {
    modifiers_first(a, b).reverse()
}
