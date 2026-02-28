# Fonts for text in VR and desktop

Place `FiraSans-Bold.ttf` here so that **text is visible in the headset** (Redis status and live feed are rasterized onto 3D quads) and for the desktop mirror.

- **VR text:** If this file is missing, the app still runs but no text quads are created in XR; you’ll only see the diagram and the colored status quad.
- Copy from the [Bevy repo](https://github.com/bevyengine/bevy/blob/main/assets/fonts/FiraSans-Bold.ttf) or use another TTF and update the path in `setup_vr_text_font` in `src/main.rs`.
