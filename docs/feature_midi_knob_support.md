# Feature: MIDI Control Knob Support

## Overview

This document specifies full support for MIDI control knobs (rotary encoders) in keebmidi. The feature adds:

1. **Directional knob detection** — interpreting CC messages as clockwise (CW) or counter-clockwise (CCW) rotation.
2. **Media / special key bindings** — extending `KeySpec` to support keys like Volume Up, Volume Down, Mute, Play/Pause, Next Track, Previous Track, etc.
3. **Per-direction action binding** — allowing a single knob to trigger different actions for CW vs CCW rotation (e.g., CW → Volume Up, CCW → Volume Down).

This builds on the existing `ControlChange` trigger and `OutputAction` model without breaking existing mappings or config compatibility.

---

## Motivation

Physical MIDI controllers (e.g., Behringer X-Touch Mini, Akai MPK Mini, DJ controllers) commonly feature rotary encoders that send CC messages. Users expect to bind knob turns to directional actions — especially system media keys like volume control. The current implementation treats CC messages as flat value-threshold triggers, which cannot distinguish rotation direction and cannot emit media key events.

---

## Background: How MIDI Knobs Work

MIDI knobs transmit **Control Change (CC)** messages. There are two common encoding schemes:

### Absolute Mode
- Value ranges from 0–127.
- Turning CW increases the value; turning CCW decreases it.
- Direction must be inferred by comparing the current value to the previous value for that controller.

### Relative Mode (Encoder Mode)
Most hardware encoders use one of these relative conventions:

| Mode               | CW values | CCW values | Center |
|--------------------|-----------|------------|--------|
| **Relative 1** (Two's complement) | 1–63      | 65–127     | 64     |
| **Relative 2** (Binary offset)    | 65–127    | 1–63       | 64     |
| **Relative 3** (Sign+magnitude)   | 1–64      | 127–65     | 0      |

In all relative modes, a single CC message encodes direction and speed (distance from center = velocity of turn).

### Implementation Requirement

The app must support **both** absolute and relative encoding. Relative mode should be auto-detected when possible, but the user must be able to override the encoding mode per-knob in the config.

---

## Data Model Changes

### 1. New `KnobMode` Enum

Add a new enum to represent knob encoding modes:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KnobMode {
    /// Infer direction from value delta (requires state tracking).
    #[default]
    Absolute,
    /// CW = 1–63, CCW = 65–127 (two's complement style).
    Relative1,
    /// CW = 65–127, CCW = 1–63 (binary offset style).
    Relative2,
    /// CW = 1–64, CCW = 127–65 (sign+magnitude style).
    Relative3,
}
```

**File:** `src/config/model.rs`

### 2. New `KnobDirection` Enum

Add a direction enum used internally during event processing:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobDirection {
    Clockwise,
    CounterClockwise,
}
```

**File:** `src/midi/decode.rs`

### 3. New `MidiTrigger::KnobRotation` Variant

Add a new trigger variant to `MidiTrigger`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiTrigger {
    // ... existing variants unchanged ...

    /// Fires when a CC knob is rotated in a specific direction.
    KnobRotation {
        channel: u8,
        controller: u8,
        direction: KnobRotationDirection,
        #[serde(default)]
        mode: KnobMode,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnobRotationDirection {
    Clockwise,
    CounterClockwise,
}
```

**File:** `src/midi/trigger.rs`

**Display impl:** Format as `Knob CW ch=1 ctrl=7` or `Knob CCW ch=1 ctrl=7`.

### 4. Extend `KeySpec` with Media / Special Keys

Add the following variants to the `KeySpec` enum:

```rust
pub enum KeySpec {
    // ... existing variants unchanged ...

    // Media keys
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MediaPlayPause,
    MediaStop,
    MediaNextTrack,
    MediaPrevTrack,

    // Brightness (platform-dependent)
    BrightnessUp,
    BrightnessDown,
}
```

**File:** `src/config/model.rs`

Update these associated functions and trait impls:

- **`KeySpec::from_name()`** — add parsing for new key names:
  - `"volumeup"` / `"volume_up"` → `KeySpec::VolumeUp`
  - `"volumedown"` / `"volume_down"` → `KeySpec::VolumeDown`
  - `"volumemute"` / `"volume_mute"` / `"mute"` → `KeySpec::VolumeMute`
  - `"playpause"` / `"play_pause"` / `"media_play_pause"` → `KeySpec::MediaPlayPause`
  - `"mediastop"` / `"media_stop"` → `KeySpec::MediaStop`
  - `"nexttrack"` / `"next_track"` / `"media_next"` → `KeySpec::MediaNextTrack`
  - `"prevtrack"` / `"prev_track"` / `"media_prev"` / `"media_previous"` → `KeySpec::MediaPrevTrack`
  - `"brightnessup"` / `"brightness_up"` → `KeySpec::BrightnessUp`
  - `"brightnessdown"` / `"brightness_down"` → `KeySpec::BrightnessDown`

- **`Display` for `KeySpec`** — return human-readable names (e.g., `"Volume Up"`, `"Play/Pause"`).

- **`KeySpec::is_modifier()`** — these new keys are **not** modifiers; no change needed.

### 5. Extend `KeyboardSink` / enigo Adapter

Map the new `KeySpec` variants to enigo `Key` values:

| KeySpec              | enigo Key                          |
|----------------------|------------------------------------|
| `VolumeUp`           | `Key::VolumeUp`                    |
| `VolumeDown`         | `Key::VolumeDown`                  |
| `VolumeMute`         | `Key::VolumeMute`                  |
| `MediaPlayPause`     | `Key::MediaPlayPause`              |
| `MediaStop`          | `Key::MediaStop`                   |
| `MediaNextTrack`     | `Key::MediaNextTrack`              |
| `MediaPrevTrack`     | `Key::MediaPrevTrack`              |
| `BrightnessUp`       | Platform-specific (XF86, keysym)   |
| `BrightnessDown`     | Platform-specific (XF86, keysym)   |

**File:** `src/actions/keyboard.rs`

**Note:** Brightness keys may not be available on all platforms. If enigo does not natively support them, use raw keycodes via `Key::Raw(keycode)` or report an unsupported-key warning. The `platform/capabilities.rs` module should be extended to report media key availability.

---

## MIDI Decode Changes

### Direction Detection Logic

Add a new function to `src/midi/decode.rs`:

```rust
use std::collections::HashMap;

/// State tracker for absolute-mode knobs.
/// Stores the last seen CC value per (channel, controller) pair.
pub struct KnobState {
    last_values: HashMap<(u8, u8), u8>,
}

impl KnobState {
    pub fn new() -> Self {
        Self {
            last_values: HashMap::new(),
        }
    }

    /// Determine rotation direction for an absolute-mode CC message.
    /// Returns None if this is the first message (no previous value to compare).
    pub fn detect_direction_absolute(
        &mut self,
        channel: u8,
        controller: u8,
        value: u8,
    ) -> Option<KnobDirection> {
        let key = (channel, controller);
        let direction = if let Some(&prev) = self.last_values.get(&key) {
            if value > prev {
                Some(KnobDirection::Clockwise)
            } else if value < prev {
                Some(KnobDirection::CounterClockwise)
            } else {
                None // no change
            }
        } else {
            None // first event, no direction yet
        };
        self.last_values.insert(key, value);
        direction
    }
}

/// Determine rotation direction for a relative-mode CC message.
pub fn detect_direction_relative(value: u8, mode: &KnobMode) -> Option<KnobDirection> {
    match mode {
        KnobMode::Relative1 => match value {
            1..=63 => Some(KnobDirection::Clockwise),
            65..=127 => Some(KnobDirection::CounterClockwise),
            _ => None,
        },
        KnobMode::Relative2 => match value {
            65..=127 => Some(KnobDirection::Clockwise),
            1..=63 => Some(KnobDirection::CounterClockwise),
            _ => None,
        },
        KnobMode::Relative3 => match value {
            1..=64 => Some(KnobDirection::Clockwise),
            65..=127 => Some(KnobDirection::CounterClockwise),
            _ => None,
        },
        KnobMode::Absolute => None, // caller should use KnobState instead
    }
}
```

### Integration with Event Processing

The `KnobState` instance must be held in the MIDI processing pipeline — either in the MIDI manager or passed through the app event system. When a CC event arrives:

1. Check if any `KnobRotation` mapping exists for that `(channel, controller)` pair.
2. If the mapping specifies `KnobMode::Absolute`, use `KnobState::detect_direction_absolute()`.
3. If the mapping specifies a relative mode, use `detect_direction_relative()`.
4. Emit a synthesized internal event carrying the detected `KnobDirection`.
5. The mapping engine then matches against `KnobRotation` triggers with the corresponding direction.

**Important:** The existing `ControlChange` trigger and matching logic must remain untouched. `KnobRotation` is a separate, higher-level trigger type that consumes the same raw CC bytes but applies directional interpretation.

---

## Matching Engine Changes

**File:** `src/midi/decode.rs` (function `event_matches_trigger`)

Add a new match arm for `KnobRotation`. Since `KnobRotation` triggers are derived (not directly decoded from raw bytes), the matching pipeline needs a small refactor:

### Proposed Approach

Introduce an intermediate event type or extend `MidiEvent`:

```rust
pub struct MidiEvent {
    pub trigger: MidiTrigger,
    pub raw_velocity: Option<u8>,
    pub raw_value: Option<u8>,
    pub timestamp_us: u64,
    /// Populated for CC events when knob rotation mappings exist.
    pub knob_direction: Option<KnobDirection>,
}
```

Then in `event_matches_trigger`, add:

```rust
(
    MidiTrigger::ControlChange {
        channel: ec,
        controller: ecc,
        ..
    },
    MidiTrigger::KnobRotation {
        channel: tc,
        controller: tcc,
        direction: td,
        ..
    },
) => {
    ec == tc && ecc == tcc
        && event.knob_direction.as_ref() == Some(&match td {
            KnobRotationDirection::Clockwise => KnobDirection::Clockwise,
            KnobRotationDirection::CounterClockwise => KnobDirection::CounterClockwise,
        })
}
```

This allows a raw CC event to match against both traditional `ControlChange` triggers (value-based) and `KnobRotation` triggers (direction-based) simultaneously.

---

## Reducer / App State Changes

**File:** `src/app/state.rs`

Add `KnobState` to `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    pub knob_state: KnobState,
}
```

Initialize `KnobState::new()` in the state constructor.

**File:** `src/app/reducer.rs`

When processing `AppEvent::MidiReceived`:

1. Before matching, if the event is a CC message, compute `knob_direction` using the state's `KnobState` and any relevant knob mode from matching mappings.
2. Attach the direction to the `MidiEvent`.
3. Proceed with normal matching (both `ControlChange` and `KnobRotation` triggers are checked).

---

## Learn Mode Changes

### Learn MIDI (for Knob Rotation)

**File:** `src/app/reducer.rs`, `src/ui/components/modal.rs`

When the user enters Learn MIDI mode and a CC event is received:

1. Detect if the CC source appears to be a knob (heuristic: multiple CC events from the same controller arriving in quick succession with incrementing or decrementing values).
2. If a knob is detected, show an additional prompt:
   - **"Knob detected on CC {controller}. Configure as knob? [Y/n]"**
3. If confirmed, prompt for direction:
   - **"Turn the knob clockwise…"** → record CW mapping.
   - **"Now turn counter-clockwise…"** → record CCW mapping.
4. Auto-detect the encoding mode from the observed values:
   - If values increment/decrement smoothly (e.g., 64→65→66), assume **Absolute**.
   - If values cluster around center (e.g., 1, 2, 63, 65, 66), assume **Relative 1**.
   - Inform the user of the detected mode and allow override.
5. Create **two** mappings (one CW, one CCW) from a single learn session, or let the user create them individually.

### Learn Action (unchanged but expanded)

The existing Learn Action flow works for knob bindings since it captures any `OutputAction`. The new media keys are available through:

- **Direct key press** (if the terminal forwards media key events — unlikely in most terminals).
- **Manual fallback selection** — the manual key list must be updated to include media/special keys.

**File:** `src/input/key_capture.rs`

Extend `MANUAL_KEY_LIST`:

```rust
pub const MANUAL_KEY_LIST: &[(&str, KeySpec)] = &[
    // ... existing entries ...
    ("Volume Up", KeySpec::VolumeUp),
    ("Volume Down", KeySpec::VolumeDown),
    ("Volume Mute", KeySpec::VolumeMute),
    ("Play / Pause", KeySpec::MediaPlayPause),
    ("Media Stop", KeySpec::MediaStop),
    ("Next Track", KeySpec::MediaNextTrack),
    ("Previous Track", KeySpec::MediaPrevTrack),
    ("Brightness Up", KeySpec::BrightnessUp),
    ("Brightness Down", KeySpec::BrightnessDown),
];
```

---

## Config Format Changes

### New Knob Rotation Mapping Example

```toml
[[mappings]]
id = "knob_vol_cw"
name = "Knob 1 CW -> Volume Up"
enabled = true

[mappings.trigger]
type = "knob_rotation"
channel = 1
controller = 7
direction = "clockwise"
mode = "absolute"

[mappings.action]
type = "key_tap"
key = "VolumeUp"

[mappings.options]
debounce_ms = 30
trigger_on_value_change_only = true

[[mappings]]
id = "knob_vol_ccw"
name = "Knob 1 CCW -> Volume Down"
enabled = true

[mappings.trigger]
type = "knob_rotation"
channel = 1
controller = 7
direction = "counter_clockwise"
mode = "absolute"

[mappings.action]
type = "key_tap"
key = "VolumeDown"

[mappings.options]
debounce_ms = 30
trigger_on_value_change_only = true
```

### Knob with Existing Action Types

CW and CCW can bind to **any** existing `OutputAction`, not just media keys:

```toml
[[mappings]]
id = "knob_scroll_cw"
name = "Knob 2 CW -> Ctrl+Plus (Zoom In)"
enabled = true

[mappings.trigger]
type = "knob_rotation"
channel = 1
controller = 10
direction = "clockwise"
mode = "relative_1"

[mappings.action]
type = "key_chord"
keys = ["Ctrl", "+"]

[[mappings]]
id = "knob_scroll_ccw"
name = "Knob 2 CCW -> Ctrl+Minus (Zoom Out)"
enabled = true

[mappings.trigger]
type = "knob_rotation"
channel = 1
controller = 10
direction = "counter_clockwise"
mode = "relative_1"

[mappings.action]
type = "key_chord"
keys = ["Ctrl", "-"]
```

### Config Versioning

This change is **backward-compatible**. Existing v1 configs contain no `knob_rotation` triggers and no media key specs, so they load without issue. However, bump the config version to **2** in newly saved files so that older builds can warn if they encounter an unknown trigger type.

**File:** `src/config/model.rs` — update `default_version()` to return `2` for new configs. Existing v1 configs should still load successfully; version is informational only.

---

## UI Changes

### Mapping List Display

**File:** `src/ui/components/mapping_list.rs`

No structural changes. The `Display` impl on `MidiTrigger` and `OutputAction` will handle formatting the new types. Knob mappings will display as:

```
Knob CW ch=1 ctrl=7  →  Key: Volume Up
Knob CCW ch=1 ctrl=7 →  Key: Volume Down
```

### Details Pane

**File:** `src/ui/components/details.rs`

When a `KnobRotation` mapping is selected, show additional fields:

- **Direction:** Clockwise / Counter-Clockwise
- **Encoding Mode:** Absolute / Relative 1 / Relative 2 / Relative 3
- **Controller:** CC number

Allow editing the encoding mode via a cycle-select (press a key to cycle through modes).

### Learn Modal

**File:** `src/ui/components/modal.rs`

Add new modal states for the knob learn flow:

- `"Knob detected. Configure as rotary knob? [Y/N]"`
- `"Turn the knob CLOCKWISE and press Enter…"`
- `"Turn the knob COUNTER-CLOCKWISE and press Enter…"`
- `"Detected mode: Absolute. Accept? [Y/N/Override]"`

---

## Platform Considerations

**File:** `src/platform/capabilities.rs`

Extend the platform capability checks to report media key support:

```rust
pub fn check_media_key_support() -> Vec<String> {
    let mut warnings = Vec::new();

    // Check if enigo can inject media keys on this platform.
    // On Linux/X11, media keys require XTest extension.
    // On Linux/Wayland, media key injection may be restricted.
    // On macOS, accessibility permissions are required.
    // On Windows, media keys work via SendInput without special permissions.

    #[cfg(target_os = "linux")]
    {
        // Wayland check
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            warnings.push(
                "Wayland detected: media key injection may be limited. \
                 Consider running under X11/XWayland for full support."
                    .to_string(),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        warnings.push(
            "macOS: ensure Accessibility permissions are granted for media key injection."
                .to_string(),
        );
    }

    warnings
}
```

---

## Testing Strategy

### Unit Tests

**File:** `src/midi/decode.rs` (new tests)

- `test_detect_direction_absolute_cw` — value increases → Clockwise.
- `test_detect_direction_absolute_ccw` — value decreases → CounterClockwise.
- `test_detect_direction_absolute_no_change` — same value → None.
- `test_detect_direction_absolute_first_event` — no previous → None.
- `test_detect_direction_relative1_cw` — value in 1–63 → Clockwise.
- `test_detect_direction_relative1_ccw` — value in 65–127 → CounterClockwise.
- `test_detect_direction_relative2_cw` — value in 65–127 → Clockwise.
- `test_detect_direction_relative2_ccw` — value in 1–63 → CounterClockwise.
- `test_detect_direction_relative3_cw` — value in 1–64 → Clockwise.
- `test_detect_direction_relative3_ccw` — value in 65–127 → CounterClockwise.
- `test_detect_direction_relative_zero` — value = 0 → None (all modes).
- `test_knob_state_tracks_multiple_controllers` — independent tracking per (channel, controller).

**File:** `src/config/model.rs` (new tests)

- `test_keyspec_from_name_media_keys` — parse all new media key name variants.
- `test_keyspec_display_media_keys` — display formatting for all new variants.
- `test_knob_mode_serde_roundtrip` — serialize/deserialize `KnobMode` variants.
- `test_knob_rotation_trigger_serde_roundtrip` — TOML round-trip for `KnobRotation`.

**File:** `src/midi/decode.rs` (extend existing tests)

- `test_trigger_matching_knob_rotation_cw` — CC event + CW direction matches CW trigger.
- `test_trigger_matching_knob_rotation_ccw` — CC event + CCW direction matches CCW trigger.
- `test_trigger_matching_knob_rotation_wrong_direction` — CW event does not match CCW trigger.
- `test_trigger_matching_cc_still_works` — existing CC threshold matching unaffected.

### Integration Tests

- Mock MIDI producer sends a sequence of CC values (e.g., 63, 64, 65, 66) for absolute mode → verify CW action fires.
- Mock MIDI producer sends relative-1 values (e.g., 1, 2, 65, 66) → verify CW and CCW actions fire correctly.
- Verify that a knob rotation mapping and a traditional CC mapping on the same controller can coexist and both fire.
- Verify media key actions reach the mock `KeyboardSink` with correct `KeySpec` variants.
- Config round-trip: save a config with knob mappings → reload → verify all fields preserved.

### Manual / Acceptance Tests

- Connect a physical MIDI controller with a rotary encoder.
- Use Learn MIDI mode → confirm knob detection prompt appears.
- Bind CW to Volume Up and CCW to Volume Down.
- Enter Run mode → turn knob → verify system volume changes.
- Bind CW to `Ctrl++` and CCW to `Ctrl+-` → verify zoom behavior in a target app.
- Edit config TOML by hand → change `mode` from `absolute` to `relative_1` → reload → verify behavior matches.

---

## Implementation Phases

### Phase 1: Data Model & Serialization
- Add `KnobMode`, `KnobRotationDirection` enums.
- Add `MidiTrigger::KnobRotation` variant.
- Add media key variants to `KeySpec`.
- Update `Display`, `from_name()`, serde impls.
- Update `MANUAL_KEY_LIST`.
- Write unit tests for all new types and serialization.

### Phase 2: Direction Detection Engine
- Implement `KnobState` for absolute mode tracking.
- Implement `detect_direction_relative()`.
- Add `knob_direction` field to `MidiEvent`.
- Write unit tests for direction detection.

### Phase 3: Matching Engine Integration
- Add `KnobRotation` match arm in `event_matches_trigger()`.
- Wire `KnobState` into `AppState`.
- Update reducer to compute knob direction on CC events.
- Write matching tests.

### Phase 4: Keyboard Output for Media Keys
- Map new `KeySpec` variants to enigo `Key` values in `keyboard.rs`.
- Add platform capability warnings for media keys.
- Test media key emission on each supported platform.

### Phase 5: Learn Mode UX for Knobs
- Add knob detection heuristic in learn MIDI flow.
- Add modal prompts for direction learning.
- Add auto-detection of encoding mode.
- Create paired CW/CCW mappings from single learn session.

### Phase 6: UI Polish
- Update details pane for knob-specific fields.
- Add encoding mode editor (cycle-select).
- Update keybind legend if new shortcuts are added.

---

## Edge Cases & Considerations

1. **Knob at limit (absolute mode):** When a knob reaches 0 or 127 and the user keeps turning, no CC events are sent. This is hardware-limited and not something the app can work around. The UI should document this behavior.

2. **Noisy encoders:** Some cheap encoders jitter, sending CW then CCW rapidly. The `debounce_ms` option on the mapping already handles this. Recommend a default debounce of `30ms` for knob mappings.

3. **Speed sensitivity:** In relative modes, the distance from center indicates speed. V1 does not need to support speed-sensitive output (e.g., "turn faster → hold key longer"), but the data model should not preclude it. The raw CC value is preserved in `MidiEvent::raw_value` for future use.

4. **Multiple knobs on same channel:** Each knob has a unique CC controller number. The `KnobState` tracks per `(channel, controller)`, so multiple knobs work independently.

5. **Mixed CC usage:** A user might have a CC mapped as both a traditional threshold trigger (e.g., "CC 7 value > 100 → fire action") and a knob rotation trigger on the same controller. Both should be able to coexist — the matching engine checks all mappings independently.

6. **Config migration:** Existing v1 configs have no `knob_rotation` triggers. Loading them into the updated app requires no migration. Saving from the updated app bumps the version to 2. If a v2 config is loaded into an older build, the unknown `knob_rotation` trigger type should be skipped with a warning per the existing config error handling policy.

---

## AI Coding Agent Notes

When implementing this feature, treat the following as hard requirements:

1. **Do not modify or remove existing `ControlChange` trigger behavior.** `KnobRotation` is an additional trigger type, not a replacement.
2. **`KnobState` must be cheap and bounded.** Use a `HashMap` limited to active controllers. Clear entries for controllers that haven't sent events in a configurable timeout (e.g., 60 seconds) to prevent unbounded growth.
3. **All new `KeySpec` variants must have round-trip serde tests.** Serialize to TOML and deserialize back; assert equality.
4. **The learn flow for knobs must be cancelable with `Esc` at every step.**
5. **Media key support must degrade gracefully.** If a platform cannot emit a media key, log a warning and skip the action — do not crash or panic.
6. **Maintain the existing abstraction boundary.** All keyboard output goes through `KeyboardSink`. All MIDI decoding goes through `decode.rs`. Do not add enigo calls or midir calls outside their designated modules.
7. **Keep config backward-compatible.** A config file with no knob mappings must load identically to before this change.
8. **Direction detection for absolute mode is stateful.** Ensure `KnobState` is reset when the MIDI device is disconnected or changed.
