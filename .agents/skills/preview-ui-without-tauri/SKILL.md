# Preview the UI Without Running Tauri

Render and drive the real built frontend in an ordinary browser, so UI changes
can be verified without OBS, a game, or a Tauri build.

## Use when

- You changed anything under `src/` and need to see it.
- You need to check a layout at a specific width, or in dark mode.
- You need to confirm a control is disabled/enabled under some state.

Do **not** use for: notification behaviour, file dialogs, window chrome
behaviour, or anything whose effect is produced by the OS. Those need the real
app — see the `verify-by-effect` skill.

## Required capabilities

- A static file server (`npx http-server`, `python -m http.server`, anything).
- Browser automation that can navigate, evaluate JavaScript in the page, and
  screenshot.

## Why a shim is needed

The frontend calls the Tauri runtime through `window.__TAURI_INTERNALS__`.
Outside Tauri that object does not exist and the app fails at startup.
`tauri-shim.js` in this directory supplies a fake one with canned responses.

---

## Procedure

1. Build the frontend from the repo root:

   ```bash
   npx ng build
   ```

   Output lands in `dist/kovobs/browser/`.

2. Copy the build to a scratch directory outside the repo, together with
   `tauri-shim.js` from this skill directory.

3. Inject the shim as the **first** thing in `<head>` of the copied
   `index.html`:

   ```html
   <head>
   <script src="/tauri-shim.js"></script>
   ```

   It must load before the app bundle, or the app starts before the bridge
   exists.

4. Serve the scratch directory and open it with browser automation. Disable
   caching (`-c-1` for `http-server`) or you will debug a stale bundle.

5. Verify by **measuring the DOM**, not only by screenshot:

   ```js
   [...document.querySelectorAll('.toggle')].map(t => {
     const b = t.querySelector('button[role=switch]');
     return t.querySelector('.toggle__label').textContent.trim()
       + ' on=' + b.getAttribute('aria-checked') + ' disabled=' + b.disabled;
   })
   ```

6. Stop the server when done, and do not leave scratch launch configs in the
   repo.

---

## Rules

1. **Add every new Tauri command to the shim's `R` map.** A command with no
   entry resolves `null` and the UI silently renders empty. The console shows
   `[shim] unhandled <cmd>`.
2. **Rebuild and re-copy after every source change.** The scratch copy is a
   snapshot; editing `src/` does nothing to it.
3. **Screenshot for layout, measure for state.** Ask the DOM whether a control
   is disabled; do not infer it from pixels.
4. **Verify narrow widths.** Emulate ~620px and assert the element sits inside
   its row and the page has no horizontal scroll. Buttons pushed off the right
   edge are the most common regression here.
5. **Reset any viewport emulation** before finishing.

## Recommendations

- To test a UI branch, stub the command's return in the shim rather than
  changing app code.
- To assert a button reached the backend, have the shim set a flag:
  `quit_app: () => { window.__QUIT_CALLED__ = true; return null; }` — then check
  the flag after clicking. Proves wiring, not just presence.
- Screenshots can time out if the window is hidden; fall back to reading the
  DOM rather than retrying blindly.

## Verification criteria

- The element exists **and** its computed state matches the intended one.
- Checked at both a wide and a narrow viewport.
- Checked in dark mode if colour or contrast changed.
- Any button added is confirmed to invoke its command, not merely to render.

## Known limits

- Notifications, dialogs, and file reveals do nothing here — the shim returns
  `null`. Verify those in the real app.
- Values shown come from the shim's canned config, not a real one; do not read
  them as evidence about real data.
