mod commands;
mod models;
mod services;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::environment::detect_flutter_environment,
            commands::environment::run_flutter_doctor,
            commands::environment::detect_android_environment,
            commands::environment::detect_ios_environment,
            commands::environment::detect_system_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
