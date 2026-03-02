# Design Document: Rust TUI CLI for Mapping MIDI Inputs to Keyboard Keys and Macros

## Overview

This document specifies a Rust-based terminal application that lets a user map **MIDI input events** (notes, CCs, etc.) to **keyboard outputs** (single keys, key chords, text input, or recorded macros). The application runs in a terminal, uses a simple TUI, and prioritizes a fast “learn” workflow:

1. Learn a MIDI input by listening for the next incoming MIDI event.
2. Learn a keyboard action by capturing the next key/chord or recording a macro.
3. Save the mapping.
4. Enter active run mode where incoming MIDI triggers emit keyboard output.

For the UI stack, the recommended choice is **`ratatui`** with **`crossterm`**. `ratatui` is a flexible Rust TUI library and does not impose a framework structure, which makes it a good fit for an agent-friendly architecture. `crossterm` provides cross-platform terminal event handling and raw mode support. For MIDI, use **`midir`**. For keyboard simulation, use **`enigo`**. ([Ratatui][1])

---

## Primary Goals

The application should:

* Run entirely as a terminal application.
* Detect and connect to one or more MIDI input devices.
* Support a **learn mode** for MIDI triggers.
* Support a **learn mode** for keyboard output:

    * single key
    * key chord (ex: `Ctrl+Shift+P`)
    * text entry
    * recorded macro (sequence of key events + delays)
* Persist mappings to disk in a human-editable config format.
* Let users enable/disable mappings individually.
* Show clear real-time status/log output.
* Be cross-platform where practical.

---

## Non-Goals for V1

To keep the initial implementation realistic, do **not** include these in V1:

* Arbitrary shell command execution.
* Mouse output mapping.
* Network-based remote control.
* Plugin system.
* Multiple profiles auto-switching by app/window.
* Deep MIDI filtering beyond the common event types.
* Background daemon/service mode.

Those can be designed later once the core mapping loop is stable.

---

## User Experience Summary

The app should feel like a hardware-control utility, not a general-purpose automation framework.

### Core user flow

1. Launch app.
2. Select MIDI input device.
3. See list of existing mappings.
4. Press **Add Mapping**.
5. App enters **Learn MIDI** mode and waits for the next MIDI event.
6. App stores that event as the trigger.
7. App enters **Learn Action** mode:

    * “Press a key”
    * or “Press a chord”
    * or “Record macro”
    * or “Type text”
8. User confirms.
9. Mapping appears in the list.
10. User saves config.
11. User switches app to **Run** mode.
12. Incoming MIDI events trigger keyboard output.

### UX principles

* Minimal screen changes.
* Always-visible status bar.
* Always-visible connection state.
* Clear modal/overlay for “learning”.
* Keyboard-only navigation.

---

## Recommended Technology Stack

### Core crates

* **`ratatui`**: rendering widgets/layout for the terminal UI. It is a lightweight TUI library and leaves app architecture up to the developer. ([Ratatui][1])
* **`crossterm`**: terminal backend, raw mode, keyboard event reading, screen control. Its event module supports keyboard/mouse/resize events, and raw mode is required for proper keyboard event handling. ([Docs.rs][2])
* **`midir`**: MIDI device enumeration and input connections. It provides MIDI input/output abstractions and an `Ignore` enum for filtering received message classes. ([Docs.rs][3])
* **`enigo`**: keyboard event simulation. It supports sending text, logical key events, and raw/physical-style key events. ([Docs.rs][4])

### Supporting crates

Recommended:

* `clap` for CLI flags
* `serde` + `toml` for config serialization
* `thiserror` for typed errors
* `anyhow` for top-level app errors
* `crossbeam-channel` (or `std::sync::mpsc`) for event routing
* `tracing` + `tracing-subscriber` for debug logs
* `uuid` or `ulid` for mapping IDs
* `indexmap` if stable ordering matters

---

## High-Level Architecture

The app should use a **single UI thread** and **message-passing from background producers**.

This is especially important because `crossterm`’s event APIs should not be called from multiple threads arbitrarily; the docs explicitly warn against mixing event reads across threads or combining incompatible event APIs. ([Docs.rs][2])

### Architecture components

1. **UI/Event Loop**

    * Owns terminal rendering.
    * Reads keyboard input from `crossterm`.
    * Processes app events from channels.
    * Updates state and redraws.

2. **MIDI Input Manager**

    * Enumerates devices.
    * Opens selected input connection(s).
    * Converts raw MIDI bytes into internal `MidiEvent`.
    * Sends `AppEvent::MidiReceived` into the main event queue.

3. **Mapping Engine**

    * Matches incoming MIDI events against configured triggers.
    * Resolves to a target action.
    * Handles debounce / rate limiting / toggle logic.

4. **Action Executor**

    * Converts app actions into keyboard output calls via `enigo`.
    * Executes macros safely and sequentially.
    * Runs on a worker thread or queued task lane so long-running macros do not freeze UI.

5. **Persistence Layer**

    * Loads config at startup.
    * Saves config on command.
    * Supports import/export in TOML.

6. **Domain State Store**

    * Central `AppState`.
    * Pure-ish mutation through actions/messages.
    * Avoid direct state mutation inside renderer.

---

## Core Internal Data Model

## MIDI trigger model

The internal trigger model should normalize raw MIDI messages into stable app-defined types.

```rust
enum MidiTrigger {
    NoteOn {
        channel: u8,
        note: u8,
        min_velocity: Option<u8>,
        max_velocity: Option<u8>,
    },
    NoteOff {
        channel: u8,
        note: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        min_value: Option<u8>,
        max_value: Option<u8>,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    PitchBend {
        channel: u8,
        min_value: Option<i16>,
        max_value: Option<i16>,
    },
}
```

### V1 recommendation

Support these first:

* `NoteOn`
* `ControlChange`

Optionally store but do not expose advanced editing for:

* `NoteOff`
* `ProgramChange`

This gives immediate utility for pads, keys, knobs, and foot pedals.

---

## Output action model

```rust
enum OutputAction {
    KeyTap(KeySpec),
    KeyChord(Vec<KeySpec>),
    Text(String),
    Macro(MacroSpec),
}
```

### Key representation

```rust
enum KeySpec {
    Char(char),
    Enter,
    Tab,
    Esc,
    Backspace,
    Space,
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Ctrl,
    Alt,
    Shift,
    Meta,
    // platform-specific extensions possible later
}
```

### Macro representation

```rust
struct MacroSpec {
    steps: Vec<MacroStep>,
    playback_mode: PlaybackMode,
}

enum MacroStep {
    KeyDown(KeySpec),
    KeyUp(KeySpec),
    KeyTap(KeySpec),
    Text(String),
    DelayMs(u64),
}

enum PlaybackMode {
    FireAndForget,
    CancelAndRestart,
    IgnoreIfRunning,
    Queue,
}
```

---

## Mapping record

```rust
struct Mapping {
    id: String,
    name: String,
    enabled: bool,
    trigger: MidiTrigger,
    action: OutputAction,
    options: MappingOptions,
}
```

```rust
struct MappingOptions {
    debounce_ms: u64,
    suppress_retrigger_while_held: bool,
    trigger_on_value_change_only: bool,
    allow_overlap: bool,
}
```

---

## Application state

```rust
struct AppState {
    mode: AppMode,
    midi_devices: Vec<MidiDeviceInfo>,
    selected_device: Option<DeviceId>,
    mappings: Vec<Mapping>,
    selected_mapping: Option<usize>,
    learn_state: Option<LearnState>,
    log_lines: Vec<String>,
    dirty: bool,
}
```

```rust
enum AppMode {
    Setup,
    Run,
    LearnMidi,
    LearnAction,
    RecordMacro,
    ConfirmDialog,
    ErrorDialog,
}
```

```rust
enum LearnState {
    WaitingForMidi,
    WaitingForSingleKey,
    WaitingForChord {
        pressed: Vec<KeySpec>,
    },
    RecordingMacro {
        started_at_ms: u64,
        steps: Vec<MacroStep>,
    },
}
```

---

## TUI Layout

The UI should be simple and remain stable across states.

### Suggested layout

* **Top bar**

    * app name
    * current mode
    * selected MIDI device
    * connection status

* **Left pane**

    * mappings list
    * name
    * trigger summary
    * action summary
    * enabled/disabled marker

* **Right pane**

    * details/editor for selected mapping
    * editable fields
    * hints

* **Bottom pane**

    * live log / event stream
    * latest MIDI event
    * latest executed action

* **Footer**

    * keybind legend

### Example keybinds

* `q`: quit
* `s`: save
* `a`: add mapping
* `e`: edit mapping
* `d`: delete mapping
* `l`: learn MIDI
* `k`: learn key/chord
* `m`: record macro
* `r`: toggle run mode
* `space`: enable/disable mapping
* `tab`: switch pane

---

## Learn Mode Design

This is the most important part of the product.

## Learn MIDI

### Behavior

When the user enters **Learn MIDI**:

* Show centered modal: “Waiting for MIDI input…”
* Suspend mapping execution for that modal only.
* Listen for next qualifying MIDI event from the selected device.
* Normalize the event into `MidiTrigger`.
* Display a confirmation preview:

    * Example: `NoteOn ch=1 note=36 vel=*`
    * Example: `CC ch=1 ctrl=64 val=*`

### Rules

* By default, ignore MIDI clock / active sensing / system realtime noise.
* Accept only user-meaningful messages in learn mode.
* If multiple events arrive rapidly, take the **first eligible** event and ignore the rest until confirm/cancel.
* Allow `Esc` to cancel.

Using `midir`’s filtering via its ignore settings is appropriate for cutting down noisy non-musical messages. ([Docs.rs][3])

---

## Learn Key / Chord

### Behavior

When the user enters **Learn Action** for key/chord:

* Show modal: “Press a key or chord…”
* Put terminal in raw mode.
* Capture key events from `crossterm`.
* If only one non-modifier key is pressed, store as `KeyTap`.
* If modifiers + key are pressed together, store as `KeyChord`.

### Recommended capture heuristic

* Track pressed modifiers (`Ctrl`, `Alt`, `Shift`, `Meta`) while waiting.
* Finalize when the first non-modifier key is received.
* Compose action as `[modifiers..., key]`.

### Important limitation

Terminal input capture is constrained by the terminal and OS. Some key combinations may be intercepted before the app sees them. Because of that, the UI should always provide a **manual edit fallback** where the user can choose keys from a list if learn mode cannot observe the intended combo. `crossterm` requires raw mode for correct keyboard event handling. ([Docs.rs][2])

---

## Record Macro

### Behavior

When the user enters macro recording:

* Show modal: “Recording macro… Press Enter to stop, Esc to cancel.”
* Capture key down/up events where available.
* Insert `DelayMs` steps between captured actions.
* On stop, normalize the steps.

### Macro normalization rules

* Collapse duplicate modifier noise.
* Drop zero-length delays.
* Clamp excessively small delays to a minimum (ex: 10–20ms).
* Optionally cap total macro duration (ex: 30s).
* Show the final macro in human-readable form.

### Suggested UX

A macro preview should render like:

```text
Ctrl down
K tap
Delay 120ms
Ctrl up
Text "hello"
Enter tap
```

---

## Matching and Execution Semantics

The mapping engine should be deterministic.

### Matching rules

* Only enabled mappings participate.
* Mappings are checked in stable order.
* Multiple mappings may match the same MIDI event unless explicitly blocked.
* If overlap is disabled, the first match wins.

### Debounce

Each mapping may define `debounce_ms`:

* If the same trigger fires repeatedly within the window, ignore it.
* Default: `50ms` for NoteOn, `0ms` for discrete buttons if desired.

### Knob/CC behavior

For `ControlChange`, support two modes:

1. **Threshold Trigger**

    * Fires when value enters a range.
2. **Edge Trigger**

    * Fires only when value crosses from outside range to inside range.

V1 should ship with threshold mode plus `trigger_on_value_change_only`.

### Macro concurrency

Macros should not run directly on the UI thread.

A dedicated executor queue should:

* take `OutputAction`
* serialize execution
* track running macro IDs
* obey `PlaybackMode`

---

## Keyboard Output Layer

The output layer should abstract `enigo` behind an internal trait.

```rust
trait KeyboardSink {
    fn key_down(&mut self, key: &KeySpec) -> Result<()>;
    fn key_up(&mut self, key: &KeySpec) -> Result<()>;
    fn key_tap(&mut self, key: &KeySpec) -> Result<()>;
    fn text(&mut self, text: &str) -> Result<()>;
}
```

### Why abstract it?

* Testability
* Mocking
* Platform-specific fallbacks
* Future alternate backends

`enigo` supports text input and both logical and raw key-style injection APIs, which maps well to this abstraction. ([Docs.rs][5])

### Platform note

`enigo` is cross-platform, but backend behavior differs by OS. Its docs note Linux support paths and that some Linux backends are experimental. The app should therefore perform capability checks and show a warning banner when the active backend may be limited. ([Docs.rs][4])

---

## Config File Format

Use **TOML** for readability.

### Example

```toml
version = 1
selected_device = "MPK mini IV"

[[mappings]]
id = "map_01"
name = "Kick Pad -> Space"
enabled = true

[mappings.trigger]
type = "note_on"
channel = 1
note = 36

[mappings.action]
type = "key_tap"
key = "Space"

[mappings.options]
debounce_ms = 50
suppress_retrigger_while_held = true
trigger_on_value_change_only = false
allow_overlap = true

[[mappings]]
id = "map_02"
name = "Pedal -> Ctrl+Shift+P"
enabled = true

[mappings.trigger]
type = "cc"
channel = 1
controller = 64
min_value = 1
max_value = 127

[mappings.action]
type = "key_chord"
keys = ["Ctrl", "Shift", "P"]

[mappings.options]
debounce_ms = 100
suppress_retrigger_while_held = false
trigger_on_value_change_only = true
allow_overlap = false
```

### Config requirements

* Human-editable
* Strictly versioned
* Unknown fields ignored only if safe
* Invalid records skipped with warning, not fatal for whole file

---

## CLI Interface

The app is a TUI app, but a few flags are useful.

### Recommended flags

* `--config <path>`
* `--device <name-or-id>`
* `--run`
* `--no-run`
* `--verbose`
* `--dump-default-config`

### Behavior

* If `--run` is passed, start directly in run mode after loading config.
* If `--device` is passed, try auto-selecting a matching MIDI device.
* If no config exists, create an empty in-memory config and prompt to save.

---

## Threading Model

### Required separation

* **UI thread**

    * terminal input
    * render
    * app state mutation

* **MIDI callback producer**

    * receives MIDI bytes
    * decodes and sends `AppEvent::MidiReceived`

* **Action executor worker**

    * runs macros and keyboard output

### Event channel design

```rust
enum AppEvent {
    Tick,
    TerminalKey(TerminalKeyEvent),
    TerminalResize(u16, u16),
    MidiReceived(MidiEvent),
    ActionCompleted(String),
    ActionFailed(String, String),
    DeviceChanged,
}
```

### Rationale

This keeps the UI responsive and avoids mixing terminal I/O with asynchronous MIDI input in unsafe ways. `crossterm` specifically cautions that event reads should stay coordinated on one thread. ([Docs.rs][2])

---

## Error Handling

Errors should be visible, recoverable, and non-destructive.

### Categories

* MIDI device not found
* MIDI connection lost
* Config parse failure
* Config save failure
* Keyboard injection failure
* Unsupported key spec on current platform
* Terminal initialization failure

### Rules

* Device disconnect should not crash the app.
* Show status banner: “MIDI device disconnected.”
* Allow reselect/reconnect from UI.
* Saving errors should preserve dirty state.
* Always restore terminal state on panic or fatal error.

Because raw mode changes terminal behavior substantially, terminal cleanup is critical on exit. `crossterm` documents that raw mode disables normal line buffering and default handling of keys like Ctrl+C. ([Docs.rs][6])

---

## Security and Safety

This app emits synthetic keyboard input, which can be disruptive.

### Safety requirements

* No shell execution in V1.
* No hidden background behavior.
* Explicit run mode indicator.
* Clear “armed” vs “editing” mode.
* Optional panic button hotkey inside the app (`F12` or similar) to suspend mapping execution.
* Log every triggered mapping.

### Future-safe stance

If command execution is ever added later, it should be:

* disabled by default
* behind explicit config opt-in
* visibly marked as unsafe

---

## Testing Strategy

## Unit tests

Cover:

* MIDI byte parsing into `MidiTrigger`
* Trigger matching rules
* Debounce logic
* Macro normalization
* Config serialization/deserialization

## Integration tests

Use:

* mock MIDI event producer
* mock keyboard sink

Verify:

* incoming MIDI produces expected `OutputAction`
* macros execute in correct order
* overlapping mappings obey policy
* disconnect/reconnect path works

## UI tests

At minimum:

* state reducer tests
* snapshot tests for rendered screens (optional but useful)

### Key design decision for testability

All business logic should be decoupled from both:

* `ratatui` rendering
* `enigo` side effects

---

## Suggested Module Layout

```text
src/
  main.rs
  app/
    mod.rs
    state.rs
    events.rs
    reducer.rs
  ui/
    mod.rs
    draw.rs
    components/
      mapping_list.rs
      details.rs
      status_bar.rs
      modal.rs
  midi/
    mod.rs
    manager.rs
    decode.rs
    trigger.rs
  input/
    mod.rs
    terminal.rs
    key_capture.rs
  actions/
    mod.rs
    executor.rs
    keyboard.rs
    macro.rs
  config/
    mod.rs
    model.rs
    load.rs
    save.rs
  platform/
    mod.rs
    capabilities.rs
  errors.rs
```

### Separation rules

* `ui/` renders only from `AppState`
* `reducer.rs` contains state transition logic
* `actions/keyboard.rs` contains the `KeyboardSink` trait + `enigo` adapter
* `midi/decode.rs` owns raw MIDI parsing/normalization

---

## Implementation Phases

## Phase 1: Skeleton

* Set up terminal init/restore
* Draw basic layout
* Add app loop
* Add static mapping list

## Phase 2: MIDI plumbing

* Enumerate devices
* Select a device
* Connect input
* Show live MIDI log

## Phase 3: Learn MIDI

* Add learn modal
* Capture first valid MIDI event
* Store temporary trigger

## Phase 4: Learn key/chord

* Capture terminal key events in raw mode
* Store `KeyTap` / `KeyChord`
* Add manual fallback editor

## Phase 5: Macro recording

* Record key sequence + delays
* Normalize and preview
* Save as `MacroSpec`

## Phase 6: Runtime execution

* Match incoming MIDI
* Execute actions through mock sink first
* Swap in `enigo` backend

## Phase 7: Persistence

* TOML save/load
* Dirty state
* Startup config path handling

## Phase 8: Hardening

* Error banners
* Device reconnect
* panic-safe terminal restore
* cross-platform capability warnings

---

## Acceptance Criteria

The V1 build is successful when all of the following are true:

* User can select a MIDI input device.
* User can learn a MIDI trigger from a live device.
* User can learn a single key or key chord.
* User can record a macro.
* User can save mappings to TOML.
* User can reload saved mappings on restart.
* In run mode, a mapped MIDI trigger emits the expected keyboard output.
* UI remains responsive during macro playback.
* App survives MIDI disconnect without crashing.
* Terminal state is restored cleanly on exit.

---

## Recommended Defaults

* Default config path:

    * Linux/macOS: XDG-style config dir
    * Windows: roaming config dir
* Default debounce: `50ms`
* Default macro min delay: `15ms`
* Default max macro length: `30s`
* Default trigger types shown in UI: `NoteOn`, `CC`

---

## AI Coding Agent Notes

If this document is used to drive implementation by agents, these constraints should be treated as hard requirements:

1. **Do not couple rendering to business logic.**
2. **Do not call terminal event readers from multiple threads.**
3. **Do not let macro execution block the UI loop.**
4. **Always restore terminal raw mode / alternate screen on shutdown.**
5. **All keyboard injection must go through an internal trait abstraction.**
6. **All persisted config must be schema-versioned.**
7. **Learn mode must be cancelable at any time with `Esc`.**
8. **Manual edit fallback must exist for key combos terminals cannot capture reliably.**

-

[1]: https://ratatui.rs/?utm_source=chatgpt.com "Ratatui | Ratatui"
[2]: https://docs.rs/crossterm/latest/crossterm/event/index.html?utm_source=chatgpt.com "crossterm::event - Rust"
[3]: https://docs.rs/midir?utm_source=chatgpt.com "midir - Rust"
[4]: https://docs.rs/enigo/?utm_source=chatgpt.com "enigo - Rust"
[5]: https://docs.rs/enigo/latest/enigo/trait.Keyboard.html?utm_source=chatgpt.com "Keyboard in enigo - Rust"
[6]: https://docs.rs/crossterm/latest/crossterm/terminal/index.html?utm_source=chatgpt.com "crossterm::terminal - Rust"
