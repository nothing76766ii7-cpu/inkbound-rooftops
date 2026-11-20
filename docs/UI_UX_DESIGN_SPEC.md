# Inkbound Rooftops — UI / UX Design Specification

> **Design Provenance**: The reference gameplay clip shows only a sparse HUD (red `100/100` health bar at top-left and `0/0` counter at top-center). All menus, settings tabs, touch clusters, and radial paint palettes below are original sketchbook-style extrapolations designed to match that visual language.

## 1. Color Tokens (WCAG Contrast Checked)
| Token | Hex | Usage |
|---|---|---|
| Paper | `#F5F4EF` | Menu surfaces, primary pale walls |
| Stone | `#D8D8D5` | Secondary panels, shaded plaster |
| Ink | `#25262B` | Hand-drawn borders, typography, outlines |
| Roof Orange | `#F47B38` | Active focus strokes, traversal accents, roof ridges |
| Health Red | `#B72B2B` | Top-left health bar fill, danger telegraph |
| Muted Gray | `#9C9FA0` | Disabled states, secondary captions, distant haze |
| Paint Teal | `#2A8C82` | Reveal/platform paint accent (paired with shape symbols) |

## 2. Screen State Machine (`ui::ScreenState`)
1. **Splash / Loading**: Inkbound Rooftops emblem, version tag, seizure/flash notice, progress bar.
2. **Main Menu**: Continue (if valid save exists), New Game, Free Roam (unlocked post-slice), Time Trials, Settings, Credits, Exit.
3. **Save Selection**: 3 save slots showing district name, timestamp, schema version, and overwrite/delete confirmation modal.
4. **In-Game HUD**:
   - **Top-Left**: Thin red health bar (`100/100`) with charcoal ink border.
   - **Top-Center**: Compact objective/counter (`0/3` glyphs or wraiths).
   - **Top-Right**: Optional landmark compass / pause trigger.
   - **Center**: Context reticle (diamond = slash, circle = paint brush, ringed cross = marked enemy).
   - **Bottom-Left**: Floating virtual movement joystick (configurable radius & dead zone).
   - **Bottom-Right**: Two-thumb action cluster — Jump, Slash (tap = light, swipe = heavy, hold = dimensional slash charge, drag off = cancel), Paint (tap = toggle mode, hold = radial mode/color wheel), Dash, Slide/Crouch.
5. **Pause**: Resume, Restart Checkpoint, Controls, Settings, Main Menu (freezes simulation).
6. **Settings Tabs**: Graphics, Audio, Controls, Gameplay, Accessibility, Language.
7. **Upgrades / Abilities**: Dash cooldown, pigment capacity, slash efficiency, wall-run stamina.
8. **World Map**: Stylized top-down orange-roof schematic showing visited cells, checkpoints, and objective filters.
9. **Mission Complete / Fail**: Clear time, collectibles, retry/continue actions.
10. **Tutorial Prompts & Confirmation Dialogs**: Dismissible contextual hints and error/recovery dialogs.

## 3. Accessibility & Performance Tiers
- **Accessibility**: Left-handed HUD mirror, HUD scale (`0.75x–1.4x`), opacity (`0.4–1.0`), colorblind-safe paint symbols (square = Brush, triangle = Platform, diamond = Mark, eye = Reveal), reduced motion (transitions `<100ms`, zero head-bob), reduced flashes (disables dimensional-slash monochrome invert), sensitivity X/Y, invert Y, optional gyro.
- **Graphics Presets**:
  - **Low (Target: 30 FPS on 4 GB RAM Vulkan phones)**: 0.70x render scale, 30 FPS cap, near-only outlines, reduced particles, 1-cell LOD0.
  - **Balanced (Default)**: 0.85x render scale, 30/60 FPS cap, full outlines, standard particles.
  - **High (Target: 60 FPS on 6 GB+ devices)**: 1.0x render scale, 60 FPS cap, full outlines, extended draw distance.
