# Quest 3 Support Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Meta Quest 3 as a first-class PC VR target by enabling the Touch Plus interaction profile.

**Architecture:** Conditionally enable the `XR_META_touch_controller_plus` extension at instance creation, add a third `suggest_interaction_profile_bindings` call for the Touch Plus profile, and log the active interaction profile when it changes. All changes in `src/main.rs` with doc updates.

**Tech Stack:** Rust, openxr 0.21 (`ExtensionSet::meta_touch_controller_plus`), ash 0.38

---

### Task 1: Enable the Touch Plus extension at instance creation

**Files:**
- Modify: `src/main.rs:89-107` (extension check + instance creation)

**Step 1: Add a `has_touch_plus` flag after the existing extension check**

After line 96 (`enabled_extensions.khr_vulkan_enable2 = true;`), add:

```rust
    let has_touch_plus = available_extensions.meta_touch_controller_plus;
    if has_touch_plus {
        enabled_extensions.meta_touch_controller_plus = true;
        log::info!("Quest 3 Touch Plus controller extension available");
    }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: enable XR_META_touch_controller_plus extension when available"
```

---

### Task 2: Add Touch Plus interaction profile bindings

**Files:**
- Modify: `src/main.rs:298-330` (interaction profile bindings section)

**Step 1: Add Touch Plus bindings before the existing Oculus Touch bindings**

The `has_touch_plus` variable from Task 1 is in scope. Insert before line 303 (the existing `oculus/touch_controller` binding):

```rust
    // Suggest bindings for Quest 3 Touch Plus controllers (requires the extension).
    if has_touch_plus {
        xr_instance.suggest_interaction_profile_bindings(
            xr_instance
                .string_to_path("/interaction_profiles/meta/touch_controller_plus")?,
            &[
                xr::Binding::new(
                    &left_hand_action,
                    xr_instance.string_to_path("/user/hand/left/input/grip/pose")?,
                ),
                xr::Binding::new(
                    &right_hand_action,
                    xr_instance.string_to_path("/user/hand/right/input/grip/pose")?,
                ),
            ],
        )?;
    }

```

Update the comment on the existing Oculus Touch binding (line 301-302) from:
```rust
    // Bind our pose actions to the physical controller paths.
    // We use the Oculus Touch profile for Rift CV1 Touch controllers.
```
To:
```rust
    // Oculus Touch profile — Rift CV1 / Quest 2 Touch controllers.
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add Quest 3 Touch Plus interaction profile bindings"
```

---

### Task 3: Log the active interaction profile

**Files:**
- Modify: `src/main.rs:638-670` (event polling in the frame loop)

**Step 1: Handle the `InteractionProfileChanged` event in the event loop**

In the `match event` block (line 640), add a new arm after the `EventsLost` arm (line 667):

```rust
                InteractionProfileChanged(_) => {
                    let left_path =
                        xr_instance.string_to_path("/user/hand/left")?;
                    let right_path =
                        xr_instance.string_to_path("/user/hand/right")?;
                    let left_profile =
                        session.current_interaction_profile(left_path)?;
                    let right_profile =
                        session.current_interaction_profile(right_path)?;
                    if left_profile != xr::Path::NULL {
                        log::info!(
                            "Left hand profile: {}",
                            xr_instance.path_to_string(left_profile)?
                        );
                    }
                    if right_profile != xr::Path::NULL {
                        log::info!(
                            "Right hand profile: {}",
                            xr_instance.path_to_string(right_profile)?
                        );
                    }
                }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: log active interaction profile when controllers connect"
```

---

### Task 4: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

**Step 1: Update README.md**

Change the opening line from:
```
Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1.
```
To:
```
Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link).
```

In the Prerequisites section, change:
```
- **Rift CV1** connected (HDMI + 2-3 USB sensors)
```
To:
```
- **Rift CV1** connected (HDMI + 2-3 USB sensors), **or Meta Quest 3** connected via Quest Link (USB or Air Link)
```

In the Troubleshooting table, add a new row:
```
| Quest 3 not detected via Link | Ensure Meta Quest Link app is running and set as active OpenXR runtime |
```

In the "How It Works" section, after the Constellation tracking sentence, add:
```
The Quest 3's inside-out tracking is similarly abstracted — no code changes needed between headsets.
```

**Step 2: Update CLAUDE.md**

Change the Project line from:
```
Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1.
```
To:
```
Minimal OpenXR + Vulkan VR prototype targeting the Oculus Rift CV1 and Meta Quest 3 (via Quest Link).
```

**Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add Quest 3 via Link as supported headset"
```
