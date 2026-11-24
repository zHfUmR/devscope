mod commands;
mod models;
mod services;
mod tray;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(services::cache::DetectionCache::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            tray::init(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::environment::detect_flutter_environment,
            commands::environment::run_flutter_doctor,
            commands::environment::detect_android_environment,
            commands::environment::detect_ios_environment,
            commands::environment::detect_system_info,
            commands::environment::detect_toolchains,
            commands::analysis::analyze_flutter_project,
            commands::analysis::analyze_build_log,
            commands::analysis::generate_diagnostic_report,
            commands::analysis::save_diagnostic_report,
            commands::analysis::check_flutter_outdated,
            commands::analysis::render_diagnostic_report,
            commands::analysis::load_diagnostic_report,
            commands::analysis::save_log_export,
            commands::analysis::print_current_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
