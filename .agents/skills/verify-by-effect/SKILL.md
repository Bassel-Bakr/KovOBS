# Verify by Effect, Not by Return Value

Fix bugs where an operation reports success but produces no effect.

## Terms

- **Effect** — the observable outcome the user wants (a window opens, a row
  exists, a message arrives). Not the return value.
- **Observation channel** — how you confirm the effect. Pick one before testing:

  | Effect type | Channel |
  |---|---|
  | Visual / audible | a person looks and reports back |
  | File | stat / read the path |
  | Database, queue, cache | read back from the destination, not the writer |
  | Remote service | query the destination's state, not the call's response |
  | Process | check it exists, or read what it wrote |

- **Probe** — a small throwaway program that exercises one link in isolation.
- **Reporter** — whoever can use a channel you cannot (usually the person who
  filed the bug). Only needed when the channel is human observation.

## Use when

- An action reports success and the user says it does nothing.
- Symptoms are conditional: "works sometimes", "works only while X runs".
- The change crosses a process, thread, machine, or OS boundary.
- Tests pass but the report persists.

Skip when a failing automated test is achievable — write that test instead.

---

## Mandatory rules

1. A success return is not evidence of an effect. Confirm through the
   observation channel.
2. Absence of an effect is evidence only if you know the trigger fired. If you
   cannot confirm the trigger, ask; do not conclude.
3. Automated events — task completions, timers, tool results — are never a
   reporter's answer.
4. One variable per probe.
5. When a hypothesis is disproven, say so and delete the code and comments
   written for it.
6. Do not report an effect-producing fix as working on the strength of tests.
7. Verify the probe against real data from the affected system, not invented
   inputs, when layout, encoding, or naming is involved.

---

## Procedure

### 1. Pin the symptom (1 exchange)

Get the reporter to select one:

- never starts
- starts, produces nothing
- works under A, not under B — name A and B

Do not investigate until this is settled. If they correct you later, abandon
that branch immediately rather than salvaging it.

### 2. Confirm the artifact under test

Check that the running build contains the change. Version, timestamp, or path.
Skip nothing here; a stale binary invalidates everything downstream.

### 3. Rule out configuration — timeboxed

Budget: **two commands**. Check only values readable as plain text or a normal
settings API:

- feature enabled/disabled at the platform level
- per-application permission or registration
- policies that suppress or filter the effect

If a value is opaque (binary blob, undocumented store), **stop and ask the
reporter to read it from the settings UI**. Do not attempt to decode it.

### 4. Map the chain, then bisect

List the links, trigger to effect:

```
trigger → payload built → API accepts → platform routes → handler runs
        → downstream call → effect
```

Discover links by grepping the call path for the entry symbol and following
each call one level. **Read the specific function, not the file.**

Probe the **middle** link first, then bisect. Do not walk the chain in order.

### 5. Probe one link

Write a throwaway program outside the project. It must:

- reproduce the calling conditions that could matter: thread type, initialised
  runtime/session context, process identity, process lifetime
- exercise one link
- print the fully constructed payload verbatim — command line, request body,
  markup, URL
- write results to a file, so they survive the process exiting
- stay alive past the callback, if a callback is under test

Keep **one** probe file and rewrite it between iterations. Do not accumulate
probes.

If the product's language cannot be compiled here, reproduce the same calls in
any available language, or drive the product itself with a debug entry point.

### 6. State the pass criterion as an effect

Write it before running: "a window opens at P", "row R exists", "the endpoint
received it". If your criterion is a return value, the probe proves nothing.

### 7. Get the observation

When the channel is human, in one message:

- say exactly what to look for and where
- list what **each** answer implies, before they answer
- ask for one observation, not a diagnosis

> Did a file browser open at `<path>`?
> - **Yes** → works outside the app; the difference is inside the app.
> - **No** → reproduced; the success return is false and execution context is
>   the next suspect.

**If no reporter is available:** do not claim a fix. Deliver the probe, the
exact question, and a written statement that the fix is unverified.

### 8. Fix, then re-verify with the shipped code

Copy the product's payload-building code into the probe verbatim — do not
re-implement it. Observe again through the same channel. Only then finalise.

### 9. Record the mechanism

- extract payload construction into a pure function, if one exists, and test the
  shapes that were wrong
- comment the real mechanism at the call site, naming the misleading return
- make any degraded fallback distinguishable in logs from the primary path

---

## Decision rules

| Situation | Action |
|---|---|
| Success returned, effect absent | Suspect execution context — thread, initialised session, identity, permissions — before arguments |
| Works while the app runs, fails when closed | Handler is in-process. Move the action to the platform (registered URI/protocol/handler) and delete the callback |
| Works, then stops within one session | Suspect a one-shot listener, a dropped receiver, or a pooled context that differs per call |
| Effect suppressed only under a platform mode | Find the exempt class; use it only for messages that justify interrupting. Do not attempt to bypass the policy |
| Cause is configuration, no code fix exists | Report the setting and its value, state the one in-product lever if any, and stop. Do not fight the platform |
| A library cannot express the needed field | Construct the payload directly and drop the library, if that removes more code than it adds; otherwise keep the library and report the limitation |
| Two hypotheses survive | Build the probe that separates them. Do not pick the likelier |
| 3 probe iterations without progress | Stop probing. Add logging to the product at each link, ship a debug build to the reporter, and diagnose from its output |
| Fix cannot be observed by any channel | Do not report success. State what remains unverified |

---

## Failure modes

| Failure | Detection | Avoidance |
|---|---|---|
| Validating by return code | "It returned success, so it works" | Require the channel |
| Concluding from silence | No log line, trigger unconfirmed | Ask whether they acted |
| Untestable explanation | You cannot design a probe for it | Every claim needs a probe |
| Probe diverges from product | Probe passes, product fails | Copy product code into the probe |
| Fallback masks the bug | Feature "works" via a degraded path | Assert which path ran |
| Assumed convention | Filtering by extension, depth, prefix, naming | Run against real data |
| Stale comment | Comment names a disproven cause | Rewrite comments when a hypothesis dies |
| Decoding opaque config | Hex dumps of settings blobs | Ask the reporter to read the UI |
| Unbounded source reading | Reading whole dependency files | Grep the symbol, read that function |

---

## Done when

- The effect was observed through the chosen channel, using the shipped code path.
- The failure reproduced before the fix and does not after.
- No disproven explanation survives in code or comments.
- Anything unverified is stated as unverified.

---

## Appendix: recurring silent-success patterns

Instances observed on desktop platforms. Treat as candidates to test, not
facts.

**Execution-context mismatch.** An API requires a specific threading or session
model. Called from the wrong one it may return success and do nothing. Helper
libraries often attempt the initialisation and ignore its failure. *Test:* call
it from a freshly created thread whose context has not yet been set.

**In-process callback, out-of-process event.** A callback for a notification,
tray item, or IPC message exists only while the process does. Later
interactions are routed through a separately registered handler the app has not
implemented. *Fix:* declare the action so the platform performs it directly.

**One-shot listener on a shared channel.** A wait that takes a single message
from a channel fed by both "activated" and "dismissed" ends on whichever
arrives first and drops the receiver. Symptom: works if acted on immediately,
dead afterwards.

**Policy suppression.** Focus/quiet/priority modes discard output after the API
accepted it. Usually one class is exempt. *Test:* send one of each class and
ask which appeared.

**Encoding and escaping.** Unescaped delimiters void the payload; unencoded
spaces or non-ASCII address the wrong target, silently. *Test inputs:* a space,
an ampersand, a quote, a non-ASCII character.

**Required field, opaque rejection.** Some payload classes require a field
(for example an urgent class requiring at least one action). Omission is
rejected without explanation, or rendered wrong.

**Assumed filesystem layout.** Scanning one level, or filtering by extension,
when the real layout nests or varies. Symptom: a fallback path always runs and
the feature appears to work.
