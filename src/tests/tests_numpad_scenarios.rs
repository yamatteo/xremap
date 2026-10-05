// Golden test for example/dual_numpad/config.yml: replays key sequences on both pads,
// and compares the actions with example/dual_numpad/scenarios.txt. After an intended
// change, the new output is in target/numpad_scenarios.txt, to copy over the golden file.
use crate::client::WMClient;
use crate::config::Config;
use crate::device::InputDeviceInfo;
use crate::event::{Event, KeyEvent, KeyValue};
use crate::event_handler::EventHandler;
use crate::operator_handler::build_stages;
use crate::timeout_manager::TimeoutManager;
use evdev::KeyCode as Key;
use nix::sys::timerfd::{ClockId, TimerFd, TimerFlags};
use std::fmt::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

fn make_handler(config: &Config, timer: TimerFd) -> EventHandler {
    let stages = build_stages(&config.stages, &Rc::new(TimeoutManager::new()));
    EventHandler::new(timer, &config.default_mode, Duration::from_micros(0), stages)
}

fn dev(path: &str) -> Rc<InputDeviceInfo> {
    Rc::new(InputDeviceInfo {
        name: "SIGMACHIP USB Keyboard".into(),
        path: PathBuf::from(path),
        vendor: 1,
        product: 1,
    })
}

#[derive(Clone, Copy)]
enum Step {
    P(&'static str, Key),
    R(&'static str, Key),
    Rep(&'static str, Key),
    Tick(u64),
}
use Step::*;

const L: &str = "/dev/input/by-id/numpad-left-kbd";
const LC: &str = "/dev/input/by-id/numpad-left-consumer";
const RR: &str = "/dev/input/by-id/numpad-right-kbd";
const RC: &str = "/dev/input/by-id/numpad-right-consumer";

fn tap(d: &'static str, k: Key) -> Vec<Step> {
    vec![P(d, k), R(d, k)]
}

fn scenarios() -> Vec<(&'static str, Vec<Step>)> {
    use Key as K;
    let mut s: Vec<(&'static str, Vec<Step>)> = vec![];

    // Tap every key of each pad in default mode.
    let left = [
        K::KEY_KP0,
        K::KEY_KP1,
        K::KEY_KP4,
        K::KEY_KP7,
        K::KEY_NUMLOCK,
        K::KEY_SPACE,
        K::KEY_KP2,
        K::KEY_KP5,
        K::KEY_KP8,
        K::KEY_KPSLASH,
        K::KEY_TAB,
        K::KEY_KP3,
        K::KEY_KP6,
        K::KEY_KP9,
        K::KEY_KPASTERISK,
        K::KEY_KPENTER,
        K::KEY_KPPLUS,
        K::KEY_KPMINUS,
        K::KEY_BACKSPACE,
    ];
    let right = [
        K::KEY_BACKSPACE,
        K::KEY_KPMINUS,
        K::KEY_KPPLUS,
        K::KEY_KPENTER,
        K::KEY_KPASTERISK,
        K::KEY_KP9,
        K::KEY_KP6,
        K::KEY_KP3,
        K::KEY_KPDOT,
        K::KEY_TAB,
        K::KEY_KPSLASH,
        K::KEY_KP8,
        K::KEY_KP5,
        K::KEY_KP2,
        K::KEY_NUMLOCK,
        K::KEY_KP7,
        K::KEY_KP4,
        K::KEY_KP1,
        K::KEY_KP0,
    ];
    s.push(("taps_left", left.iter().flat_map(|k| tap(L, *k)).collect()));
    s.push((
        "taps_left_consumer",
        vec![tap(LC, K::KEY_HOMEPAGE), tap(LC, K::KEY_MAIL), tap(LC, K::KEY_CALC)].concat(),
    ));
    s.push(("taps_right", right.iter().flat_map(|k| tap(RR, *k)).collect()));
    s.push((
        "taps_right_consumer",
        vec![tap(RC, K::KEY_CALC), tap(RC, K::KEY_MAIL), tap(RC, K::KEY_HOMEPAGE)].concat(),
    ));

    // Home-row mods: hold + other key, released inside (hold).
    for (name, hk) in [
        ("hold_r", (L, K::KEY_KP5)),
        ("hold_s", (L, K::KEY_KP8)),
        ("hold_t", (L, K::KEY_KPSLASH)),
    ] {
        s.push((name, vec![P(hk.0, hk.1), P(RR, K::KEY_KP2), R(RR, K::KEY_KP2), R(hk.0, hk.1)]));
    }
    for (name, hk) in [
        ("hold_e", (RR, K::KEY_KP9)),
        ("hold_n", (RR, K::KEY_KPASTERISK)),
        ("hold_i", (RR, K::KEY_KP6)),
    ] {
        s.push((name, vec![P(hk.0, hk.1), P(L, K::KEY_KP7), R(L, K::KEY_KP7), R(hk.0, hk.1)]));
    }
    // Rolls (tap).
    s.push(("roll_r_s", vec![P(L, K::KEY_KP5), P(L, K::KEY_KP8), R(L, K::KEY_KP5), R(L, K::KEY_KP8)]));
    s.push(("roll_a_r", vec![P(L, K::KEY_KP2), P(L, K::KEY_KP5), R(L, K::KEY_KP2), R(L, K::KEY_KP5)]));
    s.push((
        "roll_e_n",
        vec![
            P(RR, K::KEY_KP9),
            P(RR, K::KEY_KPASTERISK),
            R(RR, K::KEY_KP9),
            R(RR, K::KEY_KPASTERISK),
        ],
    ));
    // Rolls from a home-row mod into a layer-tap (tap): the mod key is released
    // while the layer-tap is still undecided.
    s.push(("roll_r_a", vec![P(L, K::KEY_KP5), P(L, K::KEY_KP2), R(L, K::KEY_KP5), R(L, K::KEY_KP2)]));
    s.push((
        "roll_i_o",
        vec![
            P(RR, K::KEY_KP6),
            P(RR, K::KEY_KP3),
            R(RR, K::KEY_KP6),
            R(RR, K::KEY_KP3),
        ],
    ));
    s.push((
        "roll_t_q",
        vec![
            P(L, K::KEY_KPSLASH),
            P(L, K::KEY_KP1),
            R(L, K::KEY_KPSLASH),
            R(L, K::KEY_KP1),
        ],
    ));
    // The same roll, with the layer-tap released after the mod's timeout: the mod was
    // released in time, but its release was buffered in the layer-taps stage.
    s.push((
        "roll_n_o_slow",
        vec![
            P(RR, K::KEY_KPASTERISK),
            Tick(100),
            P(RR, K::KEY_KP3),
            R(RR, K::KEY_KPASTERISK),
            Tick(120),
            R(RR, K::KEY_KP3),
        ],
    ));
    s.push((
        "roll_n_o_t",
        vec![
            P(RR, K::KEY_KPASTERISK),
            P(RR, K::KEY_KP3),
            R(RR, K::KEY_KPASTERISK),
            P(L, K::KEY_KPSLASH),
            R(RR, K::KEY_KP3),
            R(L, K::KEY_KPSLASH),
        ],
    ));
    // Hold by timeout.
    s.push(("timeout_r", vec![P(L, K::KEY_KP5), Tick(250), Rep(L, K::KEY_KP5), R(L, K::KEY_KP5)]));
    s.push((
        "timeout_r_then_key",
        vec![
            P(L, K::KEY_KP5),
            Tick(250),
            P(RR, K::KEY_KP9),
            R(RR, K::KEY_KP9),
            R(L, K::KEY_KP5),
        ],
    ));

    // Layer-tap A: hold for right-pad numbers.
    s.push((
        "layer_a_numbers",
        vec![
            P(L, K::KEY_KP2),
            P(RR, K::KEY_KP9),
            R(RR, K::KEY_KP9),
            P(RR, K::KEY_KPASTERISK),
            R(RR, K::KEY_KPASTERISK),
            R(L, K::KEY_KP2),
            P(RR, K::KEY_KP9),
            R(RR, K::KEY_KP9),
        ],
    ));
    s.push(("layer_a_all_right", {
        let mut v = vec![P(L, K::KEY_KP2), P(RR, K::KEY_KP9), R(RR, K::KEY_KP9)];
        v.extend(right.iter().flat_map(|k| tap(RR, *k)));
        v.extend(tap(RC, K::KEY_CALC));
        v.extend(tap(RC, K::KEY_MAIL));
        v.push(R(L, K::KEY_KP2));
        v
    }));
    s.push((
        "layer_a_timeout",
        vec![
            P(L, K::KEY_KP2),
            Tick(250),
            P(RR, K::KEY_KP3),
            R(RR, K::KEY_KP3),
            R(L, K::KEY_KP2),
        ],
    ));
    // Layer-tap O: hold for left-pad numbers (keymap).
    s.push((
        "layer_o_numbers",
        vec![
            P(RR, K::KEY_KP3),
            P(L, K::KEY_NUMLOCK),
            R(L, K::KEY_NUMLOCK),
            P(L, K::KEY_KPSLASH),
            R(L, K::KEY_KPSLASH),
            P(L, K::KEY_TAB),
            R(L, K::KEY_TAB),
            P(L, K::KEY_KPASTERISK),
            R(L, K::KEY_KPASTERISK),
            P(LC, K::KEY_HOMEPAGE),
            R(LC, K::KEY_HOMEPAGE),
            P(LC, K::KEY_MAIL),
            R(LC, K::KEY_MAIL),
            P(L, K::KEY_KP2),
            R(L, K::KEY_KP2),
            P(L, K::KEY_KP5),
            R(L, K::KEY_KP5),
            R(RR, K::KEY_KP3),
        ],
    ));
    // Both layers.
    s.push((
        "layer_a_then_o",
        vec![
            P(L, K::KEY_KP2),
            P(RR, K::KEY_KP9),
            R(RR, K::KEY_KP9),
            P(RR, K::KEY_KP3),
            R(RR, K::KEY_KP3),
            R(L, K::KEY_KP2),
        ],
    ));

    // Press/release mode keys.
    s.push(("kpdot_shift_right", {
        let mut v = vec![P(L, K::KEY_KPDOT)];
        v.extend(right.iter().flat_map(|k| tap(RR, *k)));
        v.push(R(L, K::KEY_KPDOT));
        v.extend(tap(RR, K::KEY_KP9));
        v
    }));
    s.push(("z_move_right", {
        let mut v = vec![P(L, K::KEY_KP3)];
        v.extend(right.iter().flat_map(|k| tap(RR, *k)));
        v.extend(vec![
            P(RR, K::KEY_KPASTERISK),
            Rep(RR, K::KEY_KPASTERISK),
            Rep(RR, K::KEY_KPASTERISK),
            R(RR, K::KEY_KPASTERISK),
        ]);
        v.extend(tap(RC, K::KEY_CALC));
        v.extend(tap(RC, K::KEY_MAIL));
        v.push(R(L, K::KEY_KP3));
        v.extend(tap(RR, K::KEY_KPASTERISK));
        v
    }));
    s.push(("space_shift_left", {
        let mut v = vec![P(RR, K::KEY_SPACE)];
        v.extend(left.iter().flat_map(|k| tap(L, *k)));
        v.push(R(RR, K::KEY_SPACE));
        v
    }));
    s.push(("slash_move_left", {
        let mut v = vec![P(RR, K::KEY_KP2)];
        v.extend(left.iter().flat_map(|k| tap(L, *k)));
        v.push(R(RR, K::KEY_KP2));
        v
    }));
    // Shift via home-row while in move mode: hold Z, S, N (arrow).
    s.push((
        "shift_arrow",
        vec![
            P(L, K::KEY_KP3),
            P(L, K::KEY_KP8),
            Tick(250),
            P(RR, K::KEY_KPASTERISK),
            R(RR, K::KEY_KPASTERISK),
            R(L, K::KEY_KP8),
            R(L, K::KEY_KP3),
        ],
    ));
    // Ctrl-a style chord: T held + A tap.
    s.push((
        "ctrl_a",
        vec![
            P(L, K::KEY_KPSLASH),
            P(L, K::KEY_KP2),
            R(L, K::KEY_KP2),
            R(L, K::KEY_KPSLASH),
        ],
    ));
    // Mode changes while a key is held.
    s.push((
        "held_across_mode",
        vec![
            P(RR, K::KEY_KP9),
            P(L, K::KEY_KPDOT),
            R(RR, K::KEY_KP9),
            R(L, K::KEY_KPDOT),
            P(RR, K::KEY_KP8),
            P(L, K::KEY_KP3),
            Tick(250),
            R(RR, K::KEY_KP8),
            R(L, K::KEY_KP3),
        ],
    ));
    // Repeat of plain keys.
    s.push((
        "repeat_plain",
        vec![
            P(L, K::KEY_KP4),
            Rep(L, K::KEY_KP4),
            Rep(L, K::KEY_KP4),
            R(L, K::KEY_KP4),
        ],
    ));
    // Meta holds: X and DOT.
    s.push(("hold_x_meta", vec![P(L, K::KEY_KP6), P(RR, K::KEY_KP9), R(RR, K::KEY_KP9), R(L, K::KEY_KP6)]));
    s.push(("hold_dot_meta", vec![P(RR, K::KEY_KP5), P(L, K::KEY_KP7), R(L, K::KEY_KP7), R(RR, K::KEY_KP5)]));
    // Move-mode layer-taps: Z (right pad arrows) and SLASH (left pad move mode).
    s.push((
        "layer_z_move_right",
        vec![
            P(L, K::KEY_KP3),
            P(RR, K::KEY_KPASTERISK),
            R(RR, K::KEY_KPASTERISK),
            P(RR, K::KEY_KP6),
            R(RR, K::KEY_KP6),
            R(L, K::KEY_KP3),
            P(RR, K::KEY_KPASTERISK),
            R(RR, K::KEY_KPASTERISK),
        ],
    ));
    s.push((
        "layer_z_timeout",
        vec![
            P(L, K::KEY_KP3),
            Tick(250),
            P(RR, K::KEY_KP8),
            R(RR, K::KEY_KP8),
            R(L, K::KEY_KP3),
        ],
    ));
    s.push((
        "layer_slash_move_left",
        vec![P(RR, K::KEY_KP2), P(L, K::KEY_KP5), R(L, K::KEY_KP5), R(RR, K::KEY_KP2)],
    ));
    s.push(("layer_o_all_left", {
        let mut v = vec![P(RR, K::KEY_KP3), P(L, K::KEY_KP7), R(L, K::KEY_KP7)];
        v.extend(left.iter().flat_map(|k| tap(L, *k)));
        v.extend(tap(LC, K::KEY_HOMEPAGE));
        v.extend(tap(LC, K::KEY_MAIL));
        v.push(R(RR, K::KEY_KP3));
        v
    }));
    // func: hold Q (left KP1) for the right side, SEMICOLON (right KPENTER) for the left side.
    s.push(("func_right", {
        let mut v = vec![P(L, K::KEY_KP1), P(RR, K::KEY_KPSLASH), R(RR, K::KEY_KPSLASH)];
        v.extend(right.iter().flat_map(|k| tap(RR, *k)));
        v.extend(tap(RC, K::KEY_CALC));
        v.extend(tap(RC, K::KEY_MAIL));
        v.push(R(L, K::KEY_KP1));
        v
    }));
    s.push(("func_left", {
        let mut v = vec![P(RR, K::KEY_KPENTER), P(L, K::KEY_KP1), R(L, K::KEY_KP1)];
        v.extend(left.iter().flat_map(|k| tap(L, *k)));
        v.extend(tap(LC, K::KEY_HOMEPAGE));
        v.extend(tap(LC, K::KEY_MAIL));
        v.push(R(RR, K::KEY_KPENTER));
        v
    }));
    // KPDOT in numbers_left.
    s.push((
        "kpdot_in_numbers_left",
        vec![
            P(RR, K::KEY_KP3),
            P(L, K::KEY_KPDOT),
            R(L, K::KEY_KPDOT),
            P(L, K::KEY_KP0),
            R(L, K::KEY_KP0),
            R(RR, K::KEY_KP3),
        ],
    ));

    // Only the timeout makes a hold, so these wait past it after the first press.
    let held = [
        "hold_r",
        "hold_s",
        "hold_t",
        "hold_e",
        "hold_n",
        "hold_i",
        "layer_a_numbers",
        "layer_a_all_right",
        "layer_o_numbers",
        "layer_a_then_o",
        "kpdot_shift_right",
        "z_move_right",
        "space_shift_left",
        "slash_move_left",
        "ctrl_a",
        "shift_arrow",
        "hold_x_meta",
        "hold_dot_meta",
        "layer_z_move_right",
        "layer_slash_move_left",
        "layer_o_all_left",
        "func_right",
        "func_left",
        "kpdot_in_numbers_left",
    ];
    for (name, steps) in &mut s {
        if held.contains(name) {
            steps.insert(1, Tick(250));
        }
    }
    s
}

fn fmt_actions(actions: &[crate::action::Action]) -> String {
    let mut out = String::new();
    for a in actions {
        let line = format!("{a:?}");
        let line = line
            .replace("KeyEvent(KeyEvent { key: ", "")
            .replace(", value: ", ":")
            .replace(" })", "");
        write!(out, "{line} ").unwrap();
    }
    out
}

#[test]
fn test_dual_numpad_scenarios() {
    let yaml = std::fs::read_to_string("example/dual_numpad/config.yml").unwrap();
    let mut report = String::new();
    for (name, steps) in scenarios() {
        let config = crate::tests::parse_config_for_test(&yaml);
        let timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty()).unwrap();
        let mut handler = make_handler(&config, timer);
        let mut wm = WMClient::new("none", Box::new(crate::client::null_client::NullClient), false);
        writeln!(report, "== {name}").unwrap();
        for step in steps {
            let ev = match step {
                P(d, k) => Event::KeyEvent(dev(d), KeyEvent::new(k, KeyValue::Press)),
                R(d, k) => Event::KeyEvent(dev(d), KeyEvent::new(k, KeyValue::Release)),
                Rep(d, k) => Event::KeyEvent(dev(d), KeyEvent::new(k, KeyValue::Repeat)),
                Tick(ms) => {
                    sleep(Duration::from_millis(ms));
                    Event::Tick
                }
            };
            let label = format!("{ev:?}");
            let label = label
                .split("KeyEvent { key: ")
                .nth(1)
                .map(|s| s.replace(" })", ""))
                .unwrap_or("Tick".into());
            let actions = handler.on_events(vec![ev], &config, &mut wm).unwrap();
            writeln!(report, "  {label:<28} -> {}", fmt_actions(&actions)).unwrap();
        }
    }

    let expected = std::fs::read_to_string("example/dual_numpad/scenarios.txt").unwrap();
    if report != expected {
        std::fs::write("target/numpad_scenarios.txt", &report).unwrap();
        panic!("Output differs from example/dual_numpad/scenarios.txt, see: diff example/dual_numpad/scenarios.txt target/numpad_scenarios.txt");
    }
}

// Random mashing of the home rows of both pads, released in any order, without waiting
// for the timeout: every key must be a tap. The output must be the letters in press
// order, and nothing else.
#[test]
fn test_dual_numpad_random_mashing() {
    use Key as K;
    let keys = [
        (L, K::KEY_KP2, K::KEY_A),
        (L, K::KEY_KP5, K::KEY_R),
        (L, K::KEY_KP8, K::KEY_S),
        (L, K::KEY_KPSLASH, K::KEY_T),
        (L, K::KEY_TAB, K::KEY_D),
        (RC, K::KEY_MAIL, K::KEY_H),
        (RR, K::KEY_KPASTERISK, K::KEY_N),
        (RR, K::KEY_KP9, K::KEY_E),
        (RR, K::KEY_KP6, K::KEY_I),
        (RR, K::KEY_KP3, K::KEY_O),
    ];
    let yaml = std::fs::read_to_string("example/dual_numpad/config.yml").unwrap();
    let config = crate::tests::parse_config_for_test(&yaml);
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut rand = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };

    for round in 0..2000 {
        let timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty()).unwrap();
        let mut handler = make_handler(&config, timer);
        let mut wm = WMClient::new("none", Box::new(crate::client::null_client::NullClient), false);
        let mut held: Vec<usize> = vec![];
        let mut steps: Vec<Event> = vec![];
        let mut expected: Vec<Key> = vec![];
        let max_held = 1 + rand(3);
        for _ in 0..8 {
            if held.len() >= max_held || (!held.is_empty() && rand(2) == 0) {
                let i = held.remove(rand(held.len()));
                steps.push(Event::KeyEvent(dev(keys[i].0), KeyEvent::new(keys[i].1, KeyValue::Release)));
            }
            let free: Vec<usize> = (0..keys.len()).filter(|i| !held.contains(i)).collect();
            let i = free[rand(free.len())];
            held.push(i);
            expected.push(keys[i].2);
            steps.push(Event::KeyEvent(dev(keys[i].0), KeyEvent::new(keys[i].1, KeyValue::Press)));
        }
        while !held.is_empty() {
            let i = held.remove(rand(held.len()));
            steps.push(Event::KeyEvent(dev(keys[i].0), KeyEvent::new(keys[i].1, KeyValue::Release)));
        }

        let mut out = String::new();
        let mut pressed: Vec<Key> = vec![];
        for ev in steps.clone() {
            let actions = handler.on_events(vec![ev], &config, &mut wm).unwrap();
            for a in &actions {
                if let crate::action::Action::KeyEvent(k) = a {
                    if k.value == KeyValue::Press {
                        pressed.push(k.key);
                    }
                }
            }
            out.push_str(&fmt_actions(&actions));
        }
        if pressed != expected {
            let steps: Vec<String> = steps
                .iter()
                .map(|e| match e {
                    Event::KeyEvent(d, k) => format!("{}:{:?}:{:?}", d.path.display(), k.key, k.value),
                    _ => unreachable!(),
                })
                .collect();
            panic!("round {round}\nsteps: {steps:#?}\nexpected: {expected:?}\noutput: {out}");
        }
    }
}
