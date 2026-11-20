//! Desktop developer runner for rapid iteration (`cargo run -p app_android`).

#[cfg(not(target_os = "android"))]
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let save_dir = std::env::temp_dir().join("inkbound_rooftops_dev_saves");
    let event_loop = winit::event_loop::EventLoop::new().expect("Failed to create desktop event loop");
    inkbound_rooftops::run_event_loop(event_loop, save_dir);
}

#[cfg(target_os = "android")]
fn main() {}
