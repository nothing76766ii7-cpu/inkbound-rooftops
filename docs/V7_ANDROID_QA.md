# Android V7 acceptance checklist

1. Launch: main menu has readable PLAY / RESUME, HOW TO PLAY, SETTINGS, RESTART CHECKPOINT.
2. Open HOW TO PLAY; tap NEXT three times and PREVIOUS; close returns to the originating menu.
3. Open SETTINGS; toggle REDUCED MOTION; close and reopen; verify persisted setting after autosave.
4. Start game. Point center crosshair at a vertical wall within 38m; it should gain a teal ring.
5. Press HOOK with at least 18 pigment; an orange tether should appear, player should pull toward anchor; tap again to detach.
6. Repeat hook test against roof/floor (should not hook) and out-of-range wall (should not hook).
7. Cut a world structure using dimensional slash. Confirm fragments actually split and collide, not just a visual slash line.
8. On impact verify visible camera shake and roll; in reduced motion mode verify shake suppressed.
9. Check menu button hit boxes on phone landscape orientations and different aspect ratios.
10. Run cargo fmt --check, cargo test --workspace, cargo clippy --workspace and Android build locally.
