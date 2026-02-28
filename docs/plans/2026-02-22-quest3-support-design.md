# Quest 3 Support Design: PC VR via Quest Link

**Date:** 2026-02-22
**Status:** Approved
**Scope:** Add Meta Quest 3 as a first-class PC VR target via Quest Link

## Overview

The existing prototype targets the Oculus Rift CV1. The Meta Quest 3, when connected via Quest Link (USB or Air Link), uses the same Meta PC OpenXR runtime. The runtime already maps Quest 3 Touch Plus controllers to the legacy `oculus/touch_controller` profile as a fallback, so the current code works without changes. This design makes Quest 3 a first-class citizen by adding the native Touch Plus interaction profile.

## Approach: Add Touch Plus Interaction Profile

### Extension Enablement

At OpenXR instance creation, conditionally enable `XR_META_touch_controller_plus` if the runtime advertises it. This is a no-op on systems without Quest 3 support (e.g., pure Rift CV1, SteamVR runtime).

```
if available_extensions.meta_touch_controller_plus.is_some() {
    enabled_extensions.meta_touch_controller_plus = true;
}
```

### Interaction Profile Bindings

Add a third `suggest_interaction_profile_bindings` call for `/interaction_profiles/meta/touch_controller_plus`, but only when the extension is enabled (referencing a profile from a disabled extension is an error).

Binding chain (in order of specificity):
1. `meta/touch_controller_plus` — Quest 3 Touch Plus (if extension enabled)
2. `oculus/touch_controller` — Rift CV1 / Quest 2 Touch
3. `khr/simple_controller` — universal fallback

Same grip pose paths for all profiles:
- `/user/hand/left/input/grip/pose`
- `/user/hand/right/input/grip/pose`

### Logging

After the first `sync_actions` call, query and log the active interaction profile so the user knows which controller mapping the runtime selected.

### Documentation

Update README and CLAUDE.md to list Quest 3 via Link as a supported headset.

## What This Does NOT Include

- **Color space handling** (`XR_FB_color_space`) — Not relevant until we render real content instead of solid colors.
- **Quest 3-specific inputs** (trigger curl, slide, thumb proximity) — Only grip poses are used in the current prototype.
- **Standalone Quest 3 support** (native Android) — Out of scope; this is PC VR only.
- **Passthrough / mixed reality** — Future feature, not part of this change.

## Compatibility

- **Rift CV1:** Unaffected. The Touch Plus extension is simply not available on the Oculus runtime with a CV1, so it's skipped.
- **Quest 2 via Link:** Also works — falls through to `oculus/touch_controller`.
- **SteamVR runtime:** The Meta extension won't be available; falls through to `oculus/touch_controller` or `khr/simple_controller`.

## Files Modified

- `src/main.rs` — Extension check, interaction profile binding, logging (~15-20 lines)
- `README.md` — Add Quest 3 to supported headsets
- `CLAUDE.md` — Update project description
