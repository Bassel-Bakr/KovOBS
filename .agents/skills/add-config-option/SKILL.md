# Add a Config Option

Add a user-facing setting end to end, without leaving a half-wired option or
breaking existing installs.

## Use when

Adding any persisted setting: a toggle, a path, an interval, a mode.

---

## Files to touch

| File | Change | Always? |
|---|---|---|
| `src-tauri/src/config.rs` | field on the config struct + its `Default` | yes |
| `src/app/models/config.ts` | matching field on the TS interface | yes |
| `src/app/services/config.service.ts` | value in `getEmptyConfig()` | yes |
| `src/app/home/home.component.html` | the control | yes, if user-facing |
| `src-tauri/resources/default-config.json` | **only if** the shipped value differs from the Rust `Default` | rarely |
| consumers (`kovobs.rs`, `cmds/`) | nothing, if they take the config struct | rarely |

**Do not add the key to `default-config.json` when it matches the Rust
default.** Every config struct carries `#[serde(default)]`, so a missing key
already falls back. Adding it creates a second copy that can drift.

---

## Rules

1. **New options must default to preserving current behaviour.** A setting that
   gates something already shipping defaults to on; a new interruption defaults
   to off.
2. **Hand-write `Default` when any field is not the type's zero value.**
   `#[derive(Default)]` gives `false`/`0`/`""`, which silently disables a
   feature for everyone upgrading.
3. **Never restate a nested struct's default in the parent's `Default`.** Use
   `Default::default()` and let the nested type own the value, or the two will
   disagree depending on whether the key is present in the file.
4. **Pass the config struct to consumers, not individual fields.** A function
   taking `&NotificationsConfig` absorbs new options; one taking
   `(sound: bool, urgent: bool)` forces every call site to change.
5. **Decide gating inside the consumer.** Put `if !config.enabled { return; }`
   in the function, not at each call site.
6. **Add a test that a config missing the key loads with the intended value.**

---

## Test template

```rust
/// Configs written before this option existed must keep the old behaviour.
#[test]
fn new_option_defaults_to_on() {
    let config = load_json("no-key", r#"{ "trim": false }"#);
    assert!(config.notifications.failures);
}

/// One key present must not reset its siblings to the type's zero value.
#[test]
fn a_partial_object_keeps_the_other_defaults() {
    let config = load_json("partial", r#"{ "notifications": { "sound": false } }"#);
    assert!(config.notifications.enabled);
    assert!(!config.notifications.sound);
}
```

---

## Verification criteria

- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passes, including a
  new default test.
- `npx ng build` passes — the TS interface and Rust struct agree.
- The control renders and reflects the value (see `preview-ui-without-tauri`).
- Dependent controls disable correctly when their parent option is off.
- A config file **without** the new key loads and behaves as before.

## Known duplication

`src/app/models/config.ts` and `getEmptyConfig()` are a hand-maintained mirror
of the Rust structs; nothing prevents drift beyond the build failing on a type
mismatch. Generating them from Rust (`ts-rs`, or a Tauri type-generation
plugin) would remove both files from this list. Not done — it adds a build step.
