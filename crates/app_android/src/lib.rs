//! Inkbound Rooftops — Authoritative Rust application entry for Android (`android_main`)
//! and desktop development testing (PRD v3.0 — Real Architectural Slicing + Cinematic Rig).

mod brush_hook;
use brush_hook::{BrushHook, HookAction};
use ai::{InkWraith, WraithState};
use audio::{AudioEngine, AudioOutput, SfxEvent};
use engine_core::{FixedTimestepClock, GameSettings, FIXED_DT};
use gameplay_combat::{AttackKind, CombatState, SlashConfig};
use gameplay_paint::{PaintActionResult, PaintSystem};
use gameplay_player::{MovementConfig, PlayerState};
use glam::Vec2;
use persistence::{SaveManager, SaveSlot};
use platform_input::InputRouter;
use render_wgpu::SketchRenderer;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use ui::{build_ui_vertices, hit_test_menu, HudSnapshot, MenuAction, ScreenState, SettingsTab};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, Touch, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};
use world_streaming::WorldStreamer;

/// Complete runtime game state cleanly decoupled from platform window lifecycle.
pub struct GameApp {
    window: Option<Arc<Window>>,
    renderer: Option<SketchRenderer>,
    last_frame: Instant,
    clock: FixedTimestepClock,
    settings: GameSettings,
    screen: ScreenState,
    input: InputRouter,
    last_pressed_buttons: Vec<platform_input::TouchButton>,
    turn_inertia: Vec2,
    player: PlayerState,
    movement_cfg: MovementConfig,
    combat: CombatState,
    slash_cfg: SlashConfig,
    paint: PaintSystem,
    brush_hook: BrushHook,
    tutorial_page: usize,
    tutorial_return: ScreenState,
    impact_phase: f32,
    world: WorldStreamer,
    enemies: Vec<InkWraith>,
    audio: AudioEngine,
    audio_output: AudioOutput,
    hook_reel_sfx_cd: f32,
    save_manager: SaveManager,
}

impl GameApp {
    pub fn new(save_dir: PathBuf) -> Self {
        let (mut world, anchors, puzzles) = WorldStreamer::build_vertical_slice();

        let mut enemies = Vec::new();
        for cell in world.cells.values() {
            for sp in &cell.enemy_spawns {
                enemies.push(InkWraith::new(
                    sp.id,
                    sp.cell_id,
                    sp.position,
                    sp.patrol_a,
                    sp.patrol_b,
                ));
            }
        }

        let mut paint = PaintSystem::default();
        paint.platform_anchors = anchors;
        paint.puzzles = puzzles;

        let save_manager = SaveManager::new(save_dir);
        let mut player = PlayerState::default();
        let mut settings = GameSettings::default();

        // Restore player state, settings, and deterministic world-cut history (PRD v3.0 Gate 9)
        if let Ok(slot) = save_manager.load(0) {
            player = slot.player_state;
            settings = slot.settings;
            world.replay_cut_history(&slot.world_cuts);
        }

        Self {
            window: None,
            renderer: None,
            last_frame: Instant::now(),
            clock: FixedTimestepClock::default(),
            settings,
            screen: ScreenState::MainMenu,
            input: InputRouter::default(),
            last_pressed_buttons: Vec::new(),
            turn_inertia: Vec2::ZERO,
            player,
            movement_cfg: MovementConfig::default(),
            combat: CombatState::default(),
            slash_cfg: SlashConfig::default(),
            paint,
            brush_hook: BrushHook::default(),
            tutorial_page: 0,
            tutorial_return: ScreenState::MainMenu,
            impact_phase: 0.0,
            world,
            enemies,
            audio: AudioEngine::default(),
            audio_output: AudioOutput::new(),
            hook_reel_sfx_cd: 0.0,
            save_manager,
        }
    }

    fn autosave(&self) {
        let mut slot = SaveSlot::new(0, self.world.active_district_name(), self.player.clone());
        slot.settings = self.settings.clone();
        slot.world_cuts = self.world.cut_history.clone();
        slot.persistent_paint = self
            .paint
            .strokes
            .iter()
            .filter(|s| s.permanent)
            .cloned()
            .collect();
        if let Err(err) = self.save_manager.save_atomic(&slot) {
            log::warn!("Autosave failed: {err}");
        }
    }

    fn step_simulation(&mut self) {
        self.impact_phase += FIXED_DT;
        let frame_input = self.input.sample(FIXED_DT);
        self.last_pressed_buttons = frame_input.pressed_buttons.clone();

        // Spring-damped turn inertia for cinematic first-person weapon sway
        self.turn_inertia = (self.turn_inertia + frame_input.look_delta * 1.4).clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
        self.turn_inertia *= (1.0 - 9.0 * FIXED_DT).max(0.0);

        if frame_input.pause_pressed {
            self.screen = match self.screen {
                ScreenState::InGameHud => {
                    self.autosave();
                    self.input.clear_pointers();
                    ScreenState::Pause
                }
                _ => ScreenState::InGameHud,
            };
            self.audio.play_sfx(SfxEvent::UiTick, &self.settings);
            return;
        }

        let paused = self.screen != ScreenState::InGameHud;
        if paused {
            return;
        }

        if self.combat.hit_stop_timer > 0.0 {
            self.combat.hit_stop_timer = (self.combat.hit_stop_timer - FIXED_DT).max(0.0);
            return;
        }

        // 1. World streaming, dynamic cut-fragment physics, and unified collider set
        self.world.update_streaming(self.player.position);
        self.world.tick_props(FIXED_DT);
        if self.world.landed_fragments_this_tick > 0 {
            self.audio.play_sfx(SfxEvent::DebrisFall, &self.settings);
            self.player.camera_kick = (self.player.camera_kick + 0.09).min(1.0);
        }

        let mut colliders = self.world.collect_colliders();
        colliders.extend(self.paint.active_platform_colliders());
        for enemy in &self.enemies {
            colliders.push(enemy.collider());
        }

        // V4: Interact paints a temporary wall anchor and launches a physics-driven brush hook.
        // Press again to detach. Hook does not replace ordinary paint strokes.
        if frame_input.interact_pressed {
            let eye = self.player.eye_position(1.0, 0.0, true);
            let look = self.player.look_direction();
            match self.brush_hook.activate(eye, look, &colliders, &mut self.player) {
                HookAction::Fired => {
                    self.audio.play_sfx(SfxEvent::HookFire, &self.settings);
                    self.audio.play_sfx(SfxEvent::HookAttach, &self.settings);
                },
                HookAction::Detached => self.audio.play_sfx(SfxEvent::HookRelease, &self.settings),
                HookAction::Missed => self.audio.play_sfx(SfxEvent::HookMiss, &self.settings),
            }
        }
        if self.brush_hook.tick(&mut self.player, frame_input.move_vec,
            frame_input.sprint, frame_input.jump_pressed, &colliders, FIXED_DT) {
            self.audio.play_sfx(SfxEvent::HookRelease, &self.settings);
        }

        self.hook_reel_sfx_cd = (self.hook_reel_sfx_cd - FIXED_DT).max(0.0);
        if self.brush_hook.anchor.is_some() && frame_input.sprint && self.hook_reel_sfx_cd <= 0.0 {
            self.audio.play_sfx(SfxEvent::HookReel, &self.settings);
            self.hook_reel_sfx_cd = 0.25;
        }

        // 2. Player traversal
        let p_events = self
            .player
            .step(&frame_input, &colliders, &self.movement_cfg, FIXED_DT);
        if p_events.jumped {
            self.audio.play_sfx(SfxEvent::Jump, &self.settings);
        }
        if p_events.wall_jumped {
            self.audio.play_sfx(SfxEvent::WallJump, &self.settings);
        }
        if p_events.dashed {
            self.audio.play_sfx(SfxEvent::Dash, &self.settings);
        }
        if p_events.landed_impact > 0.45 {
            self.audio.play_sfx(SfxEvent::LandingHard, &self.settings);
        }
        if p_events.footstep_triggered {
            self.audio.play_sfx(SfxEvent::FootstepRoof, &self.settings);
        }

        // Checkpoint proximity trigger
        for cell in self.world.cells.values() {
            for cp in &cell.checkpoints {
                if self.player.position.distance(cp.position) < 3.5
                    && self.player.checkpoint_id != cp.id
                {
                    self.player.checkpoint_id = cp.id.clone();
                    self.player.checkpoint_position = cp.position;
                    self.autosave();
                }
            }
        }

        // 3. Paint interaction
        let eye = self.player.eye_position(1.0, 0.0, true);
        let look = self.player.look_direction();
        if frame_input.paint_toggle_pressed {
            self.paint.cycle_mode();
            self.audio.play_sfx(SfxEvent::UiTick, &self.settings);
        }
        if frame_input.paint_stroke_pos.is_some() {
            match self
                .paint
                .apply_paint(eye, look, &colliders, &mut self.player.pigment)
            {
                PaintActionResult::StrokePlaced { glyph_activated } => {
                    self.audio.play_sfx(SfxEvent::PaintStroke, &self.settings);
                    if glyph_activated {
                        self.audio.play_sfx(SfxEvent::GlyphActivated, &self.settings);
                        self.player.paint_speed_buff_timer = 5.0;
                    }
                }
                PaintActionResult::PlatformActivated(_) => {
                    self.audio
                        .play_sfx(SfxEvent::PaintPlatformSpawn, &self.settings);
                }
                PaintActionResult::EnemyMarked(enemy_id) => {
                    if let Some(w) = self.enemies.iter_mut().find(|e| e.id == enemy_id) {
                        w.apply_paint_mark(8.0);
                    }
                    self.audio.play_sfx(SfxEvent::PaintStroke, &self.settings);
                }
                _ => {}
            }
        }
        self.paint.tick(FIXED_DT);
        self.player.pigment = (self.player.pigment + 4.5 * FIXED_DT).min(self.player.max_pigment);

        // 4. Deterministic Sword & Real World-Cut Combat Pipeline (PRD v3.0)
        if frame_input.dimensional_slash_released {
            self.combat.request_attack(
                AttackKind::Dimensional,
                frame_input.world_cut_angle,
                paused,
                &self.slash_cfg,
            );
        } else if frame_input.heavy_slash_pressed {
            self.combat.request_attack(
                AttackKind::Heavy,
                frame_input.world_cut_angle,
                paused,
                &self.slash_cfg,
            );
        } else if frame_input.light_slash_pressed {
            self.combat.request_attack(
                AttackKind::LightA,
                frame_input.world_cut_angle,
                paused,
                &self.slash_cfg,
            );
        }

        let marked_ids: Vec<u64> = self
            .enemies
            .iter()
            .filter(|e| e.is_marked())
            .map(|e| e.id)
            .collect();

        let combat_out = self.combat.step(
            FIXED_DT,
            eye,
            look,
            frame_input.slash_charging,
            frame_input.slash_charge_secs,
            frame_input.world_cut_previewing,
            frame_input.world_cut_angle,
            paused,
            self.settings.aim_assist,
            &colliders,
            &marked_ids,
            &self.slash_cfg,
        );

        if let Some(started) = combat_out.swing_started {
            match started {
                AttackKind::Heavy => self.audio.play_sfx(SfxEvent::SwordSwingHeavy, &self.settings),
                AttackKind::Dimensional => {
                    self.audio
                        .play_sfx(SfxEvent::DimensionalSlashRift, &self.settings);
                    self.player.camera_kick = (self.player.camera_kick + 0.55).min(1.0);
                }
                _ => self.audio.play_sfx(SfxEvent::SwordSwingLight, &self.settings),
            }
        }

        // Execute REAL 3D architectural mesh slicing when a CutPlane commits!
        if let Some(plane) = combat_out.committed_world_cut {
            let cut_count = self.world.apply_world_cut(plane, true);
            if cut_count > 0 {
                self.audio.play_sfx(SfxEvent::SlashImpact, &self.settings);
                self.audio.play_sfx(SfxEvent::StoneCrack, &self.settings);
                if cut_count >= 2 { self.audio.play_sfx(SfxEvent::DebrisFall, &self.settings); }
                // Brief strike freeze plus impact strength scaling with severed modules.
                self.combat.hit_stop_timer = (0.055 + cut_count.min(8) as f32 * 0.010).min(0.13);
                self.player.camera_kick = (self.player.camera_kick + 0.32 +
                    cut_count.min(7) as f32 * 0.085).min(1.0);
            }
        }

        for hit in combat_out.hits {
            self.audio.play_sfx(SfxEvent::SlashImpact, &self.settings);
            self.player.camera_kick = (self.player.camera_kick + 0.25).min(1.0);
            if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == hit.collider_id) {
                enemy.take_damage(hit.damage, hit.knockback, hit.triggers_split_vfx);
            }
        }

        // 5. Ink Wraith AI updates
        for enemy in &mut self.enemies {
            let dmg = enemy.tick(self.player.position, FIXED_DT);
            if dmg > 0.0 {
                self.player.apply_damage(dmg);
            }
        }

        // 6. Audio tick
        self.audio.tick(FIXED_DT);
    }

    fn build_hud_snapshot(&self) -> HudSnapshot {
        let defeated = self
            .enemies
            .iter()
            .filter(|e| e.state == WraithState::Defeated)
            .count() as u32;
        let solved = self.paint.puzzles.iter().filter(|p| p.solved).count() as u32;
        let cuts = self.world.telemetry.successful_cuts.min(99);
        let total = (self.enemies.len() + self.paint.puzzles.len()) as u32;

        HudSnapshot {
            health: self.player.health,
            max_health: self.player.max_health,
            stamina: self.player.stamina,
            max_stamina: self.player.max_stamina,
            pigment: self.player.pigment,
            max_pigment: self.player.max_pigment,
            slash_charge_ratio: self.combat.charge_ratio,
            dimensional_cooldown_ratio: (self.combat.dimensional_cooldown
                / self.slash_cfg.dimensional_cooldown.max(0.01))
            .clamp(0.0, 1.0),
            objective_done: defeated + solved + cuts,
            objective_total: total.max(defeated + solved + cuts),
            paint_mode_active: self.input.paint_mode_active,
            active_paint_mode: self.paint.active_mode,
            joystick: self.input.joystick_visual(&self.settings),
            pressed_buttons: self.last_pressed_buttons.clone(),
            hook_active: self.brush_hook.anchor.is_some(),
            hook_target_valid: self.brush_hook.can_target(self.player.eye_position(1.0, 0.0, true), self.player.look_direction(), &self.world.collect_colliders()),
            hook_cooldown_ratio: (self.brush_hook.cooldown / 0.35).clamp(0.0, 1.0),
            tutorial_page: self.tutorial_page,
        }
    }
}

impl ApplicationHandler for GameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default().with_title("Inkbound Rooftops");
            match event_loop.create_window(attrs) {
                Ok(win) => {
                    let win = Arc::new(win);
                    match pollster::block_on(SketchRenderer::new(win.clone())) {
                        Ok(renderer) => {
                            self.renderer = Some(renderer);
                            self.window = Some(win);
                            self.last_frame = Instant::now();
                        }
                        Err(e) => log::error!("Renderer init error: {e}"),
                    }
                }
                Err(e) => log::error!("Window creation failed: {e}"),
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.autosave();
        self.input.clear_pointers();
        self.renderer = None;
        self.window = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.autosave();
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(new_size.width, new_size.height);
                }
            }
            WindowEvent::Touch(Touch {
                id,
                phase,
                location,
                ..
            }) => {
                if let Some(r) = &self.renderer {
                    let norm = Vec2::new(
                        (location.x as f32 / r.size.0.max(1) as f32).clamp(0.0, 1.0),
                        (location.y as f32 / r.size.1.max(1) as f32).clamp(0.0, 1.0),
                    );
                    let aspect = r.aspect();
                    match phase {
                        TouchPhase::Started => {
                            if self.screen != ScreenState::InGameHud {
                                if let Some(action) = hit_test_menu(self.screen, norm.x, norm.y) {
                                    match action {
                                        MenuAction::ResumeOrPlay => {
                                            self.screen = ScreenState::InGameHud;
                                        }
                                        MenuAction::RestartCheckpoint => {
                                            self.player.respawn_at_checkpoint();
                                            self.screen = ScreenState::InGameHud;
                                        }
                                        MenuAction::ToggleReducedMotion => {
                                            self.settings.reduced_motion = !self.settings.reduced_motion;
                                            self.autosave();
                                        }
                                        MenuAction::ToggleSound => {
                                            self.settings.sfx_volume = if self.settings.sfx_volume < 0.01 { 0.9 } else { 0.0 };
                                            self.autosave();
                                        }
                                        MenuAction::OpenTutorial => {
                                            self.tutorial_return = self.screen;
                                            self.tutorial_page = 0;
                                            self.screen = ScreenState::Tutorial;
                                        }
                                        MenuAction::OpenSettings => {
                                            self.tutorial_return = self.screen;
                                            self.screen = ScreenState::Settings(SettingsTab::Accessibility);
                                        }
                                        MenuAction::NextTutorial => { self.tutorial_page = (self.tutorial_page + 1).min(3); }
                                        MenuAction::PreviousTutorial => { self.tutorial_page = self.tutorial_page.saturating_sub(1); }
                                        MenuAction::BackToPause => {
                                            self.screen = self.tutorial_return;
                                        }
                                        MenuAction::MainMenu => {
                                            self.screen = ScreenState::MainMenu;
                                        }
                                    }
                                    self.audio.play_sfx(SfxEvent::UiTick, &self.settings);
                                }
                            } else {
                                self.input.on_touch_down(id, norm, aspect, &self.settings);
                            }
                        }
                        TouchPhase::Moved => {
                            self.input.on_touch_move(id, norm, aspect, &self.settings);
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            self.input.on_touch_up(id, norm);
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                let pressed = state == ElementState::Pressed;
                match code {
                    KeyCode::KeyW => self.input.key_move.y = if pressed { 1.0 } else { 0.0 },
                    KeyCode::KeyS => self.input.key_move.y = if pressed { -1.0 } else { 0.0 },
                    KeyCode::KeyD => self.input.key_move.x = if pressed { 1.0 } else { 0.0 },
                    KeyCode::KeyA => self.input.key_move.x = if pressed { -1.0 } else { 0.0 },
                    KeyCode::ShiftLeft => self.input.key_sprint = pressed,
                    KeyCode::ControlLeft => self.input.key_slide = pressed,
                    KeyCode::Space => self.input.trigger_jump(pressed),
                    KeyCode::KeyQ if pressed => self.input.trigger_dash(),
                    KeyCode::KeyE if pressed => self.input.trigger_paint_toggle(),
                    KeyCode::KeyR if pressed => self.input.trigger_heavy_slash(),
                    KeyCode::KeyF if pressed => self.input.trigger_dimensional_slash(),
                    KeyCode::KeyG if pressed => self.input.trigger_interact(),
                    KeyCode::Escape if pressed => self.input.trigger_pause(),
                    _ => {}
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => match button {
                MouseButton::Left => self.input.trigger_light_slash(),
                MouseButton::Right => self.input.trigger_dimensional_slash(),
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;

                let (steps, alpha) = self.clock.advance(dt);
                for _ in 0..steps {
                    self.step_simulation();
                }

                // V10: Flush queued SFX to actual Android AAudio PCM stream.
                let (events, _haptics) = self.audio.drain_events();
                self.audio_output.enqueue(&events, self.audio.effective_sfx_gain(&self.settings));
                let aspect = self.renderer.as_ref().map_or(16.0 / 9.0, |r| r.aspect());
                let hud = self.build_hud_snapshot();
                let ui_verts = build_ui_vertices(self.screen, &hud, &self.settings, aspect);
                let mut view_mat = self.player.view_matrix(
                    alpha,
                    self.settings.head_bob,
                    self.settings.reduced_motion,
                );
                // Camera impact response: oscillatory decaying screen-space offset.
                // Respect reduced-motion accessibility setting.
                if !self.settings.reduced_motion && self.player.camera_kick > 0.015 {
                    let t = self.impact_phase;
                    let a = self.player.camera_kick * 0.048;
                    // Asymmetric multi-band recoil feels heavier than an even sine shake.
                    let kick_x = (t * 73.0).sin() * 0.72 + (t * 127.0).sin() * 0.28;
                    let kick_y = (t * 97.0).cos() * 0.65 + (t * 143.0).sin() * 0.35;
                    view_mat = glam::Mat4::from_translation(glam::Vec3::new(
                        kick_x * a * 1.6, kick_y * a * 1.25, -a * 0.9,
                    )) * glam::Mat4::from_rotation_z(
                        ((t * 61.0).sin() + (t * 113.0).cos() * 0.35) * a * 0.55
                    ) * view_mat;
                }
                let cam_pos = self.player.eye_position(
                    alpha,
                    self.settings.head_bob,
                    self.settings.reduced_motion,
                );
                let look_dir = self.player.look_direction();
                let fov = self
                    .player
                    .dynamic_fov_degrees(self.settings.fov_degrees, self.settings.reduced_motion);

                if let Some(r) = &mut self.renderer {
                    match r.render_frame(
                        view_mat,
                        cam_pos,
                        look_dir,
                        fov,
                        self.player.speed_factor,
                        self.player.bob_phase,
                        self.turn_inertia,
                        self.player.locomotion,
                        &self.world,
                        &self.enemies,
                        &self.paint,
                        &self.combat,
                        self.brush_hook.anchor,
                        self.brush_hook.tension,
                        &ui_verts,
                        &self.settings,
                    ) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let (w, h) = r.size;
                            r.resize(w, h);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => log::warn!("Render frame warning: {e}"),
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(win) = &self.window {
            win.request_redraw();
        }
    }
}

pub fn run_event_loop(event_loop: EventLoop<()>, save_dir: PathBuf) {
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = GameApp::new(save_dir);
    let _ = event_loop.run_app(&mut app);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("InkboundRooftops"),
    );

    let save_dir = app
        .internal_data_path()
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/inkbound_rooftops"));

    use winit::platform::android::EventLoopBuilderExtAndroid;
    let event_loop = EventLoop::builder()
        .with_android_app(app)
        .build()
        .expect("Failed to initialize Android event loop");
    run_event_loop(event_loop, save_dir);
}
