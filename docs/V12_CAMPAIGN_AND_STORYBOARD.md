# Inkbound Rooftops V12 — The Broken Sky

## Campaign

24 playable levels in six chapters, four stages each. The first stage is the authored onboarding district; levels 2–24 are deterministic seeded rooftop courses. The original game assets, hook system, SFX, slicing renderer, Android UI and restart modes remain present in source.

| Levels | Place and season | Terrain and weather | Narrative goal |
|---|---|---|---|
| 1–4 | Inkwell Canals / spring | canals, teal waterways, clear mornings and pockets of mist | Learn to paint, parkour and find the first sky seam |
| 5–8 | Saffron Bazaar / summer | market awnings, rooftop towers, sun and strong wind | Chase the clockkeeper over the market |
| 9–12 | Ember Foundries / autumn | slate chimneys, industrial skylines and smoky wind | Break the chains without destroying your route |
| 13–16 | Frostglass Peaks / winter | icy towers, white snow caps and falling snow | Ascend to the frozen bell |
| 17–20 | Monsoon Port / spring storm | waterside structures, dock cranes and driving rain | Paint a route through the port toward the lighthouse |
| 21–24 | Eclipse Spires / winter eclipse | elevated charcoal spires, mist and gusts | Cut the sky's chains and restore the sun |

Level titles are defined in `crates/world_streaming/src/campaign.rs` (`STAGES`). Difficulty is ramped by chapter and stage. Later tracks increase to 36 roofs, mix normal sprint-jump distances with web-only spans, use narrower landing areas, increase vertical variation, and place checkpoint beacons at every four or five safe platforms. Even protected checkpoint modules are immune to world cuts, keeping saved landings safe to respawn on.

The active region/season/weather/clock and rooftop progress appear in the game HUD. The day/night cycle lasts 10 minutes of actual gameplay per virtual day. It pauses during the menu and story screens, and its current time is included in save data. Lighting, shadow direction, sky, seasonal color grading, and fog are evaluated from this clock; rain, snow and wind particles appear where the regional weather calls for them. This is weather-specific presentation, not a full hydrology or physics system.

## Storyboard: 18 illustrated panels

Three storyboard pages open at the start of a fresh run and at the start of each new chapter (levels 5, 9, 13, 17 and 21). Each page has original vector illustration, short narrative lines, and tappable **NEXT** / **SKIP** buttons. Page 3's **PLAY** starts gameplay. All text and palettes are data-driven in `world_streaming::campaign`; the interactive UI is in `crates/ui/src/lib.rs`. A separate art reference sheet is in `assets/storyboard/chapter_storyboard.png` (the runtime uses vector geometry rather than decoding this reference PNG).

### Chapter I — Lost Ink
1. The sky has been sewn shut; only the brush can open it.
2. Follow the teal beacons across the canal roofs.
3. A sword fights shadows, while the Rift cuts architecture.

### Chapter II — The Market
1. Read the roof-map hidden in the summer sun.
2. Chain web swings to preserve momentum.
3. Beat the clockkeeper to the highest gate.

### Chapter III — Ash
1. The foundries weaken the city.
2. Fight Wraiths without demolishing the path below you.
3. Cut the iron anchor with the dedicated Rift attack.

### Chapter IV — Whiteout
1. Winter covers the high summits.
2. Cross longer gaps using the web and your own feet.
3. Follow the bell's light toward the coast.

### Chapter V — Monsoon
1. Rain washes the old map away.
2. Grapple between dock cranes and paint new footholds.
3. Find the light that points to the last city.

### Chapter VI — Eclipse
1. The final ascent starts among the ink-dark spires.
2. Fight the last Wraiths with the sword, without cutting roofs.
3. Sever the sky's chains using the Rift and restore daylight.

## Controls, slicing rules and progression

| Control | Intended result |
|---|---|
| SWORD tap | Light combo. Damage enemies and hit props; **no building cuts** |
| SWORD hold | Heavy enemy attack. **No building cuts** |
| RIFT/world-cut button | Only this attack can produce architectural cut planes; works on sliceable modules |
| HOOK / brush web | Anchor to hit-tested masonry; steer, reel, launch and detach |
| Movement + sprint | Maintain momentum into harder rooftop gaps |
| JUMP | Jump, wall jump, or launch from tether |
| PAINT | Paint strokes or temporary platforms where anchors exist |
| Restart checkpoint | Regenerate world and replay saved cuts, respawning at a protected checkpoint |
| Restart level | Regenerate the same seed with all intact architecture and fresh enemies |
| New run | New seed, begin at level 1 with chapter 1 storyboard |

The combat regression test `enemy_sword_buttons_never_emit_world_cut_planes` ensures Light and Heavy cannot commit a structural CutPlane. The Dimensional attack has its own test proving it still can. Both desktop and Android UI retain the existing RIFT/world-cut control. Normal combat can emit sparks on masonry but no world slicing.

## Limitations / testing

This is a Rust source implementation, not a tested Android APK. The generator guarantees ordered rooftops, tiered gaps and forward anchors, but **physical route completion is not automatically proven** for every seed. Playtest on real touch hardware, especially high-difficulty grapple gaps, collision and silhouette contrast in snowy levels. Particle counts are capped at 90 rain / 60 snow / 28 wind per frame near the player; confirm GPU performance on low-end Android devices.
