use crate::config::expmap_operator::TapHoldNextRelease;
use crate::device::InputDeviceInfo;
use crate::emit_handler::Emit;
use crate::event::{Event, KeyEvent};
use crate::operators::{ActiveOperator, OperatorAction, StaticOperator};
use crate::timeout_manager::TimeoutManager;
use evdev::KeyCode as Key;
use log::error;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct TapHoldNextReleaseOperator {
    tap: Vec<Key>,
    hold: Vec<Key>,
    timeout: Duration,
    timeout_button: Option<Vec<Key>>,
    timeout_manager: Rc<TimeoutManager>,
}

impl TapHoldNextReleaseOperator {
    pub fn get_ops(
        key: Key,
        config: &TapHoldNextRelease,
        timeout_manager: Rc<TimeoutManager>,
    ) -> Vec<(Key, Box<dyn StaticOperator>)> {
        vec![(
            key,
            Box::new(TapHoldNextReleaseOperator {
                tap: config.tap.clone(),
                hold: config.hold.clone(),
                timeout: config.timeout,
                timeout_button: config.timeout_button.clone(),
                timeout_manager,
            }),
        )]
    }
}

impl StaticOperator for TapHoldNextReleaseOperator {
    fn get_active_operator(&self, event: &Event) -> Box<dyn ActiveOperator> {
        if let Err(err) = self.timeout_manager.set_timeout(self.timeout) {
            error!("Failed to set_timeout: {err}");
        }

        match event {
            Event::KeyEvent(device, key_event) => Box::new(ActiveTapHoldNextReleaseOperator {
                device: device.clone(),
                key: key_event.key,
                tap: self.tap.clone(),
                hold: self.hold.clone(),
                timeout: self.timeout,
                timeout_button: self.timeout_button.clone(),
                start_inst: Instant::now(),
                buffered: vec![],
                pressed_after: HashSet::new(),
                state: State::New,
                active_hold_keys: vec![],
            }),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
enum State {
    New,
    Undecided,
    Held,
    Done,
}

#[derive(Debug)]
pub struct ActiveTapHoldNextReleaseOperator {
    device: Rc<InputDeviceInfo>,
    key: Key,
    tap: Vec<Key>,
    hold: Vec<Key>,
    timeout: Duration,
    timeout_button: Option<Vec<Key>>,
    start_inst: Instant,
    buffered: Vec<Event>,
    pressed_after: HashSet<Key>,
    state: State,
    active_hold_keys: Vec<Key>,
}

impl ActiveOperator for ActiveTapHoldNextReleaseOperator {
    fn on_press(&mut self, device: Rc<InputDeviceInfo>, key_event: &KeyEvent) -> OperatorAction {
        match &mut self.state {
            State::New => {
                self.state = State::Undecided;
                OperatorAction::Undecided
            }
            State::Undecided => {
                if key_event.key == self.key {
                    // Suppress spurious press of the trigger key while undecided
                    OperatorAction::Undecided
                } else {
                    self.pressed_after.insert(key_event.key);
                    self.buffered.push(Event::KeyEvent(device, key_event.clone()));
                    OperatorAction::Undecided
                }
            }
            State::Held => {
                if key_event.key == self.key {
                    // Suppress spurious press of the trigger key while held
                    OperatorAction::Partial(vec![], vec![])
                } else {
                    OperatorAction::Unhandled
                }
            }
            State::Done => unreachable!(),
        }
    }

    fn on_release(&mut self, device: Rc<InputDeviceInfo>, key_event: &KeyEvent) -> OperatorAction {
        match &mut self.state {
            State::New => unreachable!(),
            State::Undecided => {
                if key_event.key == self.key {
                    // Trigger key released before any post-pressed key release -> TAP!
                    self.state = State::Done;
                    let mut emit = vec![];
                    for key in &self.tap {
                        emit.push(Emit::key_press(device.clone(), *key));
                    }
                    for key in self.tap.iter().rev() {
                        emit.push(Emit::key_release(device.clone(), *key));
                    }
                    let unhandled = std::mem::take(&mut self.buffered);
                    OperatorAction::Done(emit, unhandled)
                } else if self.pressed_after.contains(&key_event.key) {
                    // A key pressed AFTER the trigger was released while trigger is held -> HOLD!
                    self.state = State::Held;
                    self.active_hold_keys = self.hold.clone();
                    let emit = self.hold.iter().map(|k| Emit::key_press(device.clone(), *k)).collect();

                    self.buffered.push(Event::KeyEvent(device, key_event.clone()));
                    let unhandled = std::mem::take(&mut self.buffered);
                    OperatorAction::Partial(emit, unhandled)
                } else {
                    // Key was pressed BEFORE the trigger key was pressed.
                    // As per KMonad: "because 'a' was already pressed when we started, so foo decides it is tapping"
                    // Buffering this release event, still undecided.
                    self.buffered.push(Event::KeyEvent(device, key_event.clone()));
                    OperatorAction::Undecided
                }
            }
            State::Held => {
                if key_event.key == self.key {
                    // Trigger key released -> release held keys!
                    self.state = State::Done;
                    let emit = self
                        .active_hold_keys
                        .iter()
                        .rev()
                        .map(|k| Emit::key_release(device.clone(), *k))
                        .collect();
                    OperatorAction::Done(emit, vec![])
                } else {
                    OperatorAction::Unhandled
                }
            }
            State::Done => unreachable!(),
        }
    }

    fn on_repeat(&mut self, device: Rc<InputDeviceInfo>, key_event: &KeyEvent) -> OperatorAction {
        match &mut self.state {
            State::New => unreachable!(),
            State::Undecided => {
                if key_event.key == self.key {
                    OperatorAction::Undecided
                } else {
                    self.buffered.push(Event::KeyEvent(device, key_event.clone()));
                    OperatorAction::Undecided
                }
            }
            State::Held => {
                if key_event.key == self.key {
                    let emit = self
                        .active_hold_keys
                        .iter()
                        .map(|k| Emit::key_repeat(device.clone(), *k))
                        .collect();
                    OperatorAction::Partial(emit, vec![])
                } else {
                    OperatorAction::Unhandled
                }
            }
            State::Done => unreachable!(),
        }
    }

    fn on_tick(&mut self) -> OperatorAction {
        match &mut self.state {
            State::New => unreachable!(),
            State::Undecided => {
                if self.start_inst.elapsed() <= self.timeout {
                    OperatorAction::Undecided
                } else {
                    // Timeout elapsed -> HOLD!
                    self.state = State::Held;
                    let hold_keys = self.timeout_button.clone().unwrap_or_else(|| self.hold.clone());
                    self.active_hold_keys = hold_keys.clone();
                    let emit = hold_keys
                        .into_iter()
                        .map(|k| Emit::key_press(self.device.clone(), k))
                        .collect();
                    let unhandled = std::mem::take(&mut self.buffered);
                    OperatorAction::Partial(emit, unhandled)
                }
            }
            State::Held => OperatorAction::Unhandled,
            State::Done => unreachable!(),
        }
    }

    fn on_other(&mut self, event: &Event) -> OperatorAction {
        match &mut self.state {
            State::New => unreachable!(),
            State::Undecided => {
                self.buffered.push(event.clone());
                OperatorAction::Undecided
            }
            State::Held => OperatorAction::Unhandled,
            State::Done => unreachable!(),
        }
    }
}
