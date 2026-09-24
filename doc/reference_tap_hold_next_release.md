## Tap-hold-next-release

`tap-hold-next-release` gives a key dual behaviour: it acts as one key when tapped, and as another when held. The decision between tap and hold is made based on **which event is released first**, making it ideal for home-row modifiers during fast rolling keystrokes.

Working since v0.15.14

### Experimental

Experimental means this feature is likely to change in the future as it's improved. This
can break configuration files in any version update of xremap, and will be noted in CHANGELOG.md.

Features available in `modmap` like: `device`, `mode`, key-to-key mapping,
multi-purpose key and press/release key don't work in `experimental_map`.
But application-specific remapping with `application` and `window` works since `v0.15.5`.

### Description

When the trigger key `K` is pressed, all subsequent events are buffered and a timer is armed.
The tap-or-hold decision is resolved by the first of these conditions that occurs:

1. **`K` is released before any post-`K` key is released** → **TAP**: the tap action is emitted as a press+release, then buffered keys are replayed.
2. **A key pressed *after* `K` is released while `K` is still held** → **HOLD**: the hold action is pressed, the buffered events are replayed, and hold is released when `K` is released.
3. **A key pressed *before* `K` is released while `K` is held** → still **undecided**, this release is buffered.
4. **Timeout expires while undecided** → **HOLD**: the `timeout_button` (or `hold` if unset) is pressed, buffered events are replayed.

The key insight is that for a fast typing roll like `K → A → K-release → A-release`,
`K` is released before `A`'s release, so the result is **tap**, not hold.
But for an intentional `K + A` chord like `K → A → A-release → K-release`,
`A`'s release comes before `K`'s, so the result is **hold**.

### Parameters

| Parameter | Required | Default | Description |
|---|---|---|---|
| `tap` | ✅ | — | Key(s) emitted when tapped |
| `hold` | ✅ | — | Key(s) held when in hold mode |
| `timeout` | ❌ | `200` | Milliseconds before auto-hold on timeout |
| `timeout_button` | ❌ | same as `hold` | Key(s) to hold when the timeout triggers (instead of `hold`) |

Both `tap` and `hold` accept a single key or a list of keys.

### Example: Capslock as Esc / Ctrl (home-row mod style)

```yml
experimental_map:
  - remap:
      capslock:
        tap_hold_next_release:
          tap: esc
          hold: leftctrl
          timeout: 200
```

#### Behavior

- **`Tap capslock`** → `Esc`
- **`Hold capslock` + press `a` + release `a` before releasing `capslock`** → `Ctrl+A`
- **Fast roll: press `capslock`, press `a`, release `capslock`, release `a`** → `Esc`, then `a` (no accidental modifier!)
- **`hold capslock` for 200ms** → activates `Ctrl` (timeout)

### Example: Space as Shift (no accidental shifts when rolling)

```yml
experimental_map:
  - remap:
      space:
        tap_hold_next_release:
          tap: space
          hold: leftshift
          timeout: 150
```

Unlike the `modmap` multi-purpose key, fast rolls like `space → a → space-release → a-release`
produce `space` then `a`, not `Shift+A`.

### Example: Timeout button (repeat while held)

Use `timeout_button` to get key-repeat when held long enough, while still acting as a modifier
when used with another key before the timeout:

```yml
experimental_map:
  - remap:
      capslock:
        tap_hold_next_release:
          tap: x
          hold: leftshift
          timeout: 200
          timeout_button: x   # After timeout: x repeats (auto-repeat by OS)
```

### Example: Kebab-case alias

The `tap-hold-next-release` key name (with hyphens) is also accepted, matching KMonad's syntax:

```yml
experimental_map:
  - remap:
      capslock:
        tap-hold-next-release:
          tap: esc
          hold: leftctrl
          timeout: 200
```

### Example: Multi-key tap and hold

```yml
experimental_map:
  - remap:
      capslock:
        tap_hold_next_release:
          tap: [leftctrl, c]   # Tap = Ctrl+C
          hold: [leftshift, leftalt]  # Hold = Shift+Alt
          timeout: 200
```

### Comparison with `modmap` multi-purpose key

| Feature | `modmap` multi-purpose key | `tap_hold_next_release` |
|---|---|---|
| Configuration section | `modmap:` | `experimental_map:` |
| Decision trigger | Next key **press** | Next key **release** |
| Fast roll safety | ❌ Can misfire | ✅ Safe (TAP on roll) |
| Timeout | ✅ | ✅ |
| `timeout_button` | ❌ | ✅ |
| Composable with `select:` | ❌ | ✅ |
| Application-specific | ❌ | ✅ |

### Relationship to KMonad

This feature is a direct implementation of KMonad's `tap-hold-next-release` button.
See the [KMonad documentation](https://github.com/kmonad/kmonad) for a thorough description with timing examples.

## References

- [KMonad multi-use buttons](https://github.com/kmonad/kmonad)
- [QMK Permissive Hold](https://docs.qmk.fm/tap_hold#permissive-hold)
- [ZMK Hold-Tap](https://zmk.dev/docs/behaviors/hold-tap)
