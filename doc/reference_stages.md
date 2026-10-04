## Stages

`stages` is an ordered list of remapping stages, followed by an optional `keymap`.
Each stage is a list of entries, and the output of one stage is the input of the next.
The output of the last stage goes to `keymap`, if there is one, and then to the output device.

```yaml
stages:
  # Stage 1: give both numpads the same logical keys
  - - device: { only: left-pad }
      remap:
        KP1: A
  # Stage 2: home-row mods on the logical keys
  - - remap:
        A:
          tap_hold_next_release: { tap: A, hold: LEFTSHIFT }
keymap:
  - remap:
      C-a: C-z
```

### Stage entries

An entry has the same fields as an entry of the old `modmap` and `experimental_map` sections:

| Field | Description |
|---|---|
| `remap` | Map from trigger key to operator |
| `chords` | [Chords](reference_chords.md) |
| `name` | Ignored, for readability |
| `application`, `window` | Only apply in matching windows |
| `device` | Only apply to keys from matching devices |
| `mode` | Only apply in the given mode(s) |

Every operator can be used in any stage:

- key to key(s): `A: B`, `A: [B, C]`
- [multi-purpose key](reference_multipurpose_key.md): `{ held: ..., alone: ... }`
- [press/release key](reference_press_release_key.md): `{ press: ..., release: ..., repeat: ..., skip_key_event: ... }`
- [tap-hold-next-release](reference_tap_hold_next_release.md), [double tap](reference_double_tap.md),
  [oneshot](reference_oneshot.md), [throttle](reference_throttle.md), [select](reference_select.md)

### How a stage works

- An operator becomes active when its trigger key is pressed, and stays active until the trigger is released.
  So the release always matches the press, even if the mode or the window changes while the key is held.
- Entries in the same stage don't see each other's output. To remap the output of an entry, put the second
  remap in a later stage.
- If several entries match the pressed key, the first one to decide wins. Key-to-key, multi-purpose and
  press/release keys decide immediately, so the first matching entry wins, as in the old `modmap`.
- An active operator sees new events before they are remapped by other entries of its stage.
  For example, a multi-purpose key's `interruptable` is matched against the physical key.
- Events go through the stages one at a time: everything an event produces passes through all later
  stages and keymap before the next event is handled. A mode set by a stage (e.g. a layer-tap's
  `hold: { set_mode: ... }`) applies to the events after it, in all stages.

### The old sections

`experimental_map` and `modmap` still work. They become two stages, `experimental_map` first and
`modmap` second, which is the order they were always applied in. They can't be combined with `stages`.

Configs split over several files: the stages of all files are concatenated, in file order.

### Behaviour changes compared to the old modmap

- A remapped key is released as what it was pressed as, after a mode change. Before, keys could get stuck.
- `interruptable` of a multi-purpose key is matched against the physical key, not against what
  another modmap entry remaps it to.
- Pressing a second multi-purpose key interrupts the first one, like any other key.
- A release or repeat without a press (e.g. a key held while xremap starts) passes through unmapped.

### Reloading

With `--watch=config`, the stages are rebuilt when the config file changes. Operators active at that
moment are dropped, so a key held during the reload may need to be pressed again.
