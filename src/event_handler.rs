use crate::action::Action;
use crate::client::WMClient;
use crate::config::key_combo::{KeyCombo, Modifier};
use crate::config::keymap::{build_override_table, OverrideEntry};
use crate::config::keymap_action::KeymapAction;
use crate::config::keymap_action_without_args::ActionWithoutArgs;
use crate::config::nested_remap::Remap;
use crate::config::Config;
use crate::device::InputDeviceInfo;
use crate::event::{Event, KeyEvent, RelativeEvent};
use crate::operator_handler::OperatorHandler;
use evdev::KeyCode as Key;
use log::debug;
use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{Expiration, TimerFd, TimerSetTimeFlags};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::os::fd::{AsFd, BorrowedFd};
use std::rc::Rc;
use std::time::Duration;

// This const is used to map evdev relative event codes to a pseudo keycode
pub const DISGUISED_EVENT_OFFSETTER: u16 = 59974;

// This const defines a keycode used to match any key.
// It's the offset of XHIRES_LEFTSCROLL + 1
pub const KEY_MATCH_ANY: Key = Key(DISGUISED_EVENT_OFFSETTER + 26);

pub struct EventHandler {
    // Currently pressed modifier keys, in the order they were pressed. (including virtual modifiers)
    modifiers: Vec<Key>,
    // Current nested remaps
    override_remaps: Vec<HashMap<Key, Vec<OverrideEntry>>>,
    // Key triggered on a timeout of nested remaps
    override_timeout_key: Option<Vec<Key>>,
    // Trigger a timeout of nested remaps through select(2)
    override_timer: TimerFd,
    // { set_mode: String }
    mode: String,
    // { set_mark: true }
    mark_set: bool,
    // { escape_next_key: true }
    escape_next_key: bool,
    // keypress_delay_ms
    keypress_delay: Duration,
    // Buffered actions to be dispatched. TODO: Just return actions from each function instead of using this.
    actions: Vec<Action>,
    // Event stages, applied in order before keymap.
    stages: Vec<OperatorHandler>,
}

struct TaggedActions {
    actions: Vec<KeymapAction>,
    // Whether the match was an exact match or not.
    exact_match: bool,
    // Modifiers that are currently pressed but not in the source KeyPress
    // Can only happen when match is inexact.
    extra_modifiers_pressed: HashSet<Key>,
}

impl AsFd for EventHandler {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.override_timer.as_fd()
    }
}

impl EventHandler {
    pub fn new(
        override_timer: TimerFd,
        mode: &str,
        keypress_delay: Duration,
        stages: Vec<OperatorHandler>,
    ) -> EventHandler {
        EventHandler {
            modifiers: vec![],
            override_remaps: vec![],
            override_timeout_key: None,
            override_timer,
            mode: mode.to_string(),
            mark_set: false,
            escape_next_key: false,
            keypress_delay,
            actions: vec![],
            stages,
        }
    }

    // Replace the event stages, e.g. after the config is reloaded.
    // Operators that are active in the old stages are dropped.
    pub fn set_stages(&mut self, stages: Vec<OperatorHandler>) {
        self.stages = stages;
    }

    // Pass an event through the stages from `index` onwards, depth-first: each
    // event an operator emits goes through all later stages, and then keymap,
    // before the next one. Mode changes take effect as soon as a stage emits them.
    fn run_stages(
        &mut self,
        index: usize,
        event: Event,
        config: &Config,
        wmclient: &mut WMClient,
        mouse_movement_collection: &mut Vec<RelativeEvent>,
    ) -> Result<(), Box<dyn Error>> {
        if index == self.stages.len() {
            return self.on_staged_event(event, config, wmclient, mouse_movement_collection);
        }

        // Ticks never leave a stage, so every stage must be given its own.
        let is_tick = matches!(event, Event::Tick);
        let emitted = self.stages[index].map_events(vec![event], &self.mode, &config.default_mode, wmclient);

        for event in emitted {
            if let Event::SetMode(mode) = &event {
                self.mode = mode.clone().unwrap_or_else(|| config.default_mode.clone());
            }
            self.run_stages(index + 1, event, config, wmclient, mouse_movement_collection)?;
        }

        if is_tick {
            self.run_stages(index + 1, Event::Tick, config, wmclient, mouse_movement_collection)?;
        }
        Ok(())
    }

    // Handle an Event and return Actions. This should be the only public method of EventHandler.
    pub fn on_events(
        &mut self,
        events: Vec<Event>,
        config: &Config,
        wmclient: &mut WMClient,
    ) -> Result<Vec<Action>, Box<dyn Error>> {
        debug_assert!(self.actions.is_empty());
        // a vector to collect mouse movement events to be able to send them all at once as one MouseMovementEventCollection.
        let mut mouse_movement_collection: Vec<RelativeEvent> = Vec::new();
        for event in events {
            wmclient.clear_app_class_and_title();

            if let Event::KeyEvent(_, key_event) = &event {
                debug!("=> {}: {:?}", key_event.value(), &key_event.key);
            }

            match event {
                // The nested-remap timer belongs to keymap, so it skips the stages.
                Event::OverrideTimeout => {
                    self.on_staged_event(event, config, wmclient, &mut mouse_movement_collection)?
                }
                event => self.run_stages(0, event, config, wmclient, &mut mouse_movement_collection)?,
            }
        }
        // if there is at least one mouse movement event, sending all of them as one MouseMovementEventCollection.
        if !mouse_movement_collection.is_empty() {
            self.send_action(Action::MouseMovementEventCollection(mouse_movement_collection));
        }
        Ok(self.actions.drain(..).collect())
    }

    // Apply keymap to an event that has gone through all stages.
    fn on_staged_event(
        &mut self,
        event: Event,
        config: &Config,
        wmclient: &mut WMClient,
        mouse_movement_collection: &mut Vec<RelativeEvent>,
    ) -> Result<(), Box<dyn Error>> {
        match event {
            Event::KeyEvent(device, key_event) => {
                self.on_key_event(key_event.key, key_event.value(), &device, config, wmclient)?;
            }
            Event::RelativeEvent(device, relative_event) => {
                let key = relative_event.to_disguised_key();

                // Send as disguised-event
                let was_remapped = self.on_key_event(key, PRESS, &device, config, wmclient)?;

                if !was_remapped {
                    if relative_event.code <= 2 {
                        // The relative event keycodes 1 and 2 is REL_X and REL_Y. Meaning mouse move.
                        mouse_movement_collection.push(relative_event);
                    } else {
                        self.send_action(Action::RelativeEvent(relative_event));
                    }
                }
            }

            Event::OtherEvents(event) => self.send_action(Action::InputEvent(event)),
            Event::OverrideTimeout => self.timeout_override()?,
            Event::Tick => {
                // Can be ignored. It's for operators.
            }
            Event::SetMode(mode) => {
                let mode = mode.unwrap_or_else(|| config.default_mode.clone());
                println!("mode: {mode}");
                self.mode = mode;
            }
            Event::KeymapActions(key, actions) => {
                let actions = vec![TaggedActions {
                    actions,
                    exact_match: false,
                    extra_modifiers_pressed: HashSet::new(),
                }];
                self.dispatch_actions(&actions, &key, false)?;
            }
            Event::ByPassLocal(_) => unreachable!(),
        }
        Ok(())
    }

    // Handle EventType::KEY
    // Note: virtual_modifiers, MODIFIER_KEYS and disguised keys are disjoint sets.
    fn on_key_event(
        &mut self,
        key: Key,
        value: i32,
        device: &Rc<InputDeviceInfo>,
        config: &Config,
        wmclient: &mut WMClient,
    ) -> Result<bool, Box<dyn Error>> {
        let mod_trigger = config.virtual_modifiers.contains(&key) || MODIFIER_KEYS.contains(&key);
        // Apply keymap
        let mut matched = false;
        if is_pressed(value) {
            if self.escape_next_key {
                // Modifiers are escaped, but they don't stop escaping.
                if !mod_trigger {
                    self.escape_next_key = false
                }
            } else if let Some(actions) = self.find_keymap(config, &key, device, wmclient, mod_trigger)? {
                self.dispatch_actions(&actions, &key, mod_trigger)?;
                matched = true;
            }
        }

        if key.code() >= DISGUISED_EVENT_OFFSETTER {
            // Only disguised keys care about return value.
            return Ok(matched);
        }

        if config.virtual_modifiers.contains(&key) {
            // Virtual modifiers are never sent, only updated.
            self.update_modifier(key, value);
        } else if MODIFIER_KEYS.contains(&key) {
            // Modifiers are always sent and updated. No matter if they match or not.
            self.update_modifier(key, value);
            self.send_key(&key, value);
        } else if !matched {
            // Normal keys are only sent if not matching.
            self.send_key(&key, value);
        }

        // The return value is irrelevant here.
        Ok(true)
    }

    fn timeout_override(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(keys) = &self.override_timeout_key.take() {
            for key in keys {
                self.send_key(key, PRESS);
                self.send_key(key, RELEASE);
            }
        }
        self.remove_override()
    }

    fn remove_override(&mut self) -> Result<(), Box<dyn Error>> {
        self.override_timer.unset()?;
        self.override_remaps.clear();
        self.override_timeout_key = None;
        Ok(())
    }

    fn send_keys(&mut self, keys: &Vec<Key>, value: i32) {
        for key in keys {
            self.send_key(key, value);
        }
    }

    fn send_key(&mut self, key: &Key, value: i32) {
        let event = KeyEvent::new_with(key.code(), value);
        self.send_action(Action::KeyEvent(event));
    }

    fn send_action(&mut self, action: Action) {
        self.actions.push(action);
    }

    // The return is a vector of actions, because nested remaps are included
    //  for not only the first match, but all remappings that match. This can happen
    //  if keymap-actions has more than one remap:
    //      keymap:
    //        A:
    //          - remap: { B: 1}
    //          - remap: { C: 2}
    //  or inexact modifiers:
    //      keymap:
    //        A:
    //          remap: { B: 1}
    //        C-A
    //          remap: { C: 2}
    fn find_keymap(
        &mut self,
        config: &Config,
        key: &Key,
        device: &InputDeviceInfo,
        wmclient: &mut WMClient,
        mod_trigger: bool,
    ) -> Result<Option<Vec<TaggedActions>>, Box<dyn Error>> {
        let pressed_modifiers = self
            .modifiers
            .iter()
            // If the key is a modifier-trigger, then it's regarded
            // as not pressed, because it can't be its own modifier.
            // This happens when a modifier-trigger is repeated.
            .filter(|modifier| *modifier != key)
            .copied()
            .collect();

        if !self.override_remaps.is_empty() {
            let entries: Vec<OverrideEntry> = self
                .override_remaps
                .iter()
                .flat_map(|map| map.get(key).cloned().unwrap_or_default())
                .collect();

            // Empty if the key isn't defined in any of the nested remaps, that are active.
            if !entries.is_empty() {
                if !mod_trigger {
                    self.remove_override()?;
                }

                for exact_match in [true, false] {
                    let mut remaps = vec![];
                    for entry in &entries {
                        if entry.exact_match && !exact_match {
                            continue;
                        }
                        let (extra_modifiers, missing_modifiers) =
                            Self::diff_modifiers(&pressed_modifiers, &entry.modifiers);
                        if (exact_match && !extra_modifiers.is_empty()) || !missing_modifiers.is_empty() {
                            continue;
                        }

                        let actions = TaggedActions {
                            actions: entry.actions.clone(),
                            exact_match: entry.exact_match,
                            extra_modifiers_pressed: extra_modifiers.iter().cloned().collect(),
                        };
                        let has_remap = has_remap(&entry.actions);

                        // If the first/top match was a remap, continue to find rest of the eligible remaps for this key
                        if remaps.is_empty() && !has_remap {
                            return Ok(Some(vec![actions]));
                        } else if has_remap {
                            remaps.push(actions);
                        }
                    }
                    if !remaps.is_empty() {
                        return Ok(Some(remaps));
                    }
                }
            }
            // An override remap is set but not used. Flush the pending key.
            if !mod_trigger {
                self.timeout_override()?;
            }
        }

        for key in [key, &KEY_MATCH_ANY] {
            if let Some(entries) = config.keymap_table.get(key) {
                for exact_match in [true, false] {
                    let mut remaps = vec![];
                    for entry in entries {
                        if entry.exact_match && !exact_match {
                            continue;
                        }
                        let (extra_modifiers, missing_modifiers) =
                            Self::diff_modifiers(&pressed_modifiers, &entry.modifiers);
                        if (exact_match && !extra_modifiers.is_empty()) || !missing_modifiers.is_empty() {
                            continue;
                        }
                        if let Some(window_matcher) = &entry.title {
                            if !wmclient.match_window(window_matcher) {
                                continue;
                            }
                        }

                        if let Some(application_matcher) = &entry.application {
                            if !wmclient.match_application(application_matcher) {
                                continue;
                            }
                        }
                        if let Some(device_matcher) = &entry.device {
                            if !device_matcher.matches(device) {
                                continue;
                            }
                        }
                        if let Some(modes) = &entry.mode {
                            if !modes.contains(&self.mode) {
                                continue;
                            }
                        }

                        let actions = TaggedActions {
                            actions: entry.actions.clone(),
                            exact_match: entry.exact_match,
                            extra_modifiers_pressed: extra_modifiers.iter().cloned().collect(),
                        };
                        let has_remap = has_remap(&entry.actions);

                        // If the first/top match was a remap, continue to find rest of the eligible remaps for this key
                        if remaps.is_empty() && !has_remap {
                            return Ok(Some(vec![actions]));
                        } else if has_remap {
                            remaps.push(actions)
                        }
                    }
                    if !remaps.is_empty() {
                        return Ok(Some(remaps));
                    }
                }
            }
        }
        Ok(None)
    }

    fn dispatch_actions(
        &mut self,
        actions: &Vec<TaggedActions>,
        key: &Key,
        mod_trigger: bool,
    ) -> Result<(), Box<dyn Error>> {
        for tagged_actions in actions {
            for action in &tagged_actions.actions {
                self.dispatch_action(
                    action,
                    key,
                    tagged_actions.exact_match,
                    &tagged_actions.extra_modifiers_pressed,
                    mod_trigger,
                )?;
            }
        }
        Ok(())
    }

    fn dispatch_action(
        &mut self,
        action: &KeymapAction,
        key: &Key,
        exact_match: bool,
        extra_modifiers_pressed: &HashSet<Key>,
        mod_trigger: bool,
    ) -> Result<(), Box<dyn Error>> {
        match action {
            KeymapAction::KeyCombo(key_press) => self.send_key_combo(key_press, extra_modifiers_pressed),
            KeymapAction::KeyPress(key) => self.send_key(key, PRESS),
            KeymapAction::KeyRepeat(key) => self.send_key(key, REPEAT),
            KeymapAction::KeyRelease(key) => self.send_key(key, RELEASE),
            KeymapAction::Remap(Remap {
                remap,
                timeout,
                timeout_key,
            }) => {
                let set_timeout = self.override_remaps.is_empty();
                self.override_remaps.push(build_override_table(remap, exact_match));

                let keys = match timeout_key {
                    Some(timeout_key) => Some(timeout_key.clone()),
                    None if mod_trigger => {
                        // Modifier-triggers don't need a timeout key, because they are pressed no
                        // matter if they timeout or not. Therefore they have no default.
                        None
                    }
                    None => Some(vec![*key]),
                };

                // Set timeout only if this is the first of multiple eligible remaps,
                // so the behaviour is consistent with how current normal keymap override works
                if set_timeout && keys.is_some() {
                    if let Some(timeout) = timeout {
                        let expiration = Expiration::OneShot(TimeSpec::from_duration(*timeout));
                        // TODO: Consider handling the timer in ActionDispatcher
                        self.override_timer.unset()?;
                        self.override_timer.set(expiration, TimerSetTimeFlags::empty())?;
                        self.override_timeout_key = keys;
                    }
                }
            }
            KeymapAction::Launch(command) => self.run_command(command.clone()),
            KeymapAction::SetMode(mode) => {
                self.mode = mode.clone();
                println!("mode: {mode}");
            }
            KeymapAction::SetMark(set) => self.mark_set = *set,
            KeymapAction::WithMark(key_press) => {
                self.send_key_combo(&self.with_mark(key_press), extra_modifiers_pressed)
            }
            KeymapAction::EscapeNextKey(escape_next_key) => self.escape_next_key = *escape_next_key,
            KeymapAction::Sleep(millis) => self.send_action(Action::Delay(Duration::from_millis(*millis))),
            KeymapAction::CloseByAppClass(app_class) => self.actions.push(Action::CloseByAppClass(app_class.clone())),
            KeymapAction::Action(action) => match action {
                ActionWithoutArgs::PopWindowInfo => {
                    self.send_action(Action::PopWindowInfo);
                }
                ActionWithoutArgs::PrintWindowInfo => {
                    self.send_action(Action::PrintWindowInfo);
                }
                ActionWithoutArgs::PrintWindowList => {
                    self.send_action(Action::PrintWindowList);
                }
                ActionWithoutArgs::Exit => {
                    self.send_action(Action::Exit);
                }
                ActionWithoutArgs::Reload => {
                    self.send_action(Action::Reload);
                }
                ActionWithoutArgs::ReloadConfig => {
                    self.send_action(Action::ReloadConfig);
                }
            },
        }
        Ok(())
    }

    fn send_key_combo(&mut self, key_press: &KeyCombo, extra_modifiers_pressed: &HashSet<Key>) {
        // Build extra or missing modifiers. Note that only MODIFIER_KEYS are handled
        // because virtual modifiers shouldn't make an impact outside xremap.
        let (mut extra_modifiers, mut missing_modifiers) = Self::diff_modifiers(&self.modifiers, &key_press.modifiers);
        extra_modifiers.retain(|key| MODIFIER_KEYS.contains(key) && !extra_modifiers_pressed.contains(key));
        missing_modifiers.retain(|key| MODIFIER_KEYS.contains(key));

        // Emulate the modifiers of KeyPress
        self.send_keys(&missing_modifiers, PRESS);
        self.send_keys(&extra_modifiers, RELEASE);

        // Press the main key
        self.send_key(&key_press.key, PRESS);
        self.send_key(&key_press.key, RELEASE);

        self.send_action(Action::Delay(self.keypress_delay));

        // Resurrect the original modifiers
        self.send_keys(&extra_modifiers, PRESS);
        self.send_action(Action::Delay(self.keypress_delay));
        self.send_keys(&missing_modifiers, RELEASE);
    }

    fn with_mark(&self, key_press: &KeyCombo) -> KeyCombo {
        let has_shift = key_press.modifiers.contains(&Modifier::Shift)
            || key_press.modifiers.contains(&Modifier::Key(Key::KEY_LEFTSHIFT))
            || key_press.modifiers.contains(&Modifier::Key(Key::KEY_RIGHTSHIFT));

        if self.mark_set && !has_shift {
            let mut modifiers = key_press.modifiers.clone();
            modifiers.push(Modifier::Shift);
            KeyCombo {
                key: key_press.key,
                modifiers,
            }
        } else {
            key_press.clone()
        }
    }

    fn run_command(&mut self, command: Vec<String>) {
        self.send_action(Action::Command(command));
    }

    // Return (extra_modifiers, missing_modifiers)
    fn diff_modifiers(current: &Vec<Key>, target: &[Modifier]) -> (Vec<Key>, Vec<Key>) {
        let extra_modifiers: Vec<Key> = current
            .iter()
            .filter(|modifier| !contains_modifier(target, modifier))
            .copied()
            .collect();
        let missing_modifiers: Vec<Key> = target
            .iter()
            .filter_map(|modifier| {
                if modifier.is_in(current) {
                    None
                } else {
                    match modifier {
                        Modifier::Shift => Some(Key::KEY_LEFTSHIFT),
                        Modifier::Control => Some(Key::KEY_LEFTCTRL),
                        Modifier::Alt => Some(Key::KEY_LEFTALT),
                        Modifier::Windows => Some(Key::KEY_LEFTMETA),
                        Modifier::Key(key) => Some(*key),
                    }
                }
            })
            .collect();
        (extra_modifiers, missing_modifiers)
    }

    fn update_modifier(&mut self, key: Key, value: i32) {
        if value == PRESS {
            if !self.modifiers.contains(&key) {
                self.modifiers.push(key);
            }
        } else if value == RELEASE {
            self.modifiers.retain(|&x| x != key);
        }
    }
}

fn has_remap(actions: &[KeymapAction]) -> bool {
    if actions.is_empty() {
        // When actions are empty it could either be regarded as an empty remap
        //  or no actions. In principle that shouldn't matter, but remap is
        //  implemented to gather all defined remaps, not just the first match.
        // Here we regard empty actions as non-remap, so the matching will stop
        //  here, and no actions are performed. The possibly following remaps are
        //  hence ignored.
        return false;
    }

    actions.iter().all(|x| matches!(x, KeymapAction::Remap(..)))
}

fn contains_modifier(modifiers: &[Modifier], key: &Key) -> bool {
    for modifier in modifiers {
        if match modifier {
            Modifier::Shift => key == &Key::KEY_LEFTSHIFT || key == &Key::KEY_RIGHTSHIFT,
            Modifier::Control => key == &Key::KEY_LEFTCTRL || key == &Key::KEY_RIGHTCTRL,
            Modifier::Alt => key == &Key::KEY_LEFTALT || key == &Key::KEY_RIGHTALT,
            Modifier::Windows => key == &Key::KEY_LEFTMETA || key == &Key::KEY_RIGHTMETA,
            Modifier::Key(modifier_key) => key == modifier_key,
        } {
            return true;
        }
    }
    false
}

pub static MODIFIER_KEYS: [Key; 8] = [
    // Shift
    Key::KEY_LEFTSHIFT,
    Key::KEY_RIGHTSHIFT,
    // Control
    Key::KEY_LEFTCTRL,
    Key::KEY_RIGHTCTRL,
    // Alt
    Key::KEY_LEFTALT,
    Key::KEY_RIGHTALT,
    // Windows
    Key::KEY_LEFTMETA,
    Key::KEY_RIGHTMETA,
];

// ---

fn is_pressed(value: i32) -> bool {
    value == PRESS || value == REPEAT
}

// InputEvent#value
pub const RELEASE: i32 = 0;
pub const PRESS: i32 = 1;
pub const REPEAT: i32 = 2;
