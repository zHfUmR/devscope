//! 菜单栏（状态栏）常驻图标。
//!
//! macOS 的菜单栏图标和桌面图标是两套完全不同的规则：
//!
//! - 只吃一张图，Tauri 会把它的**高度归一成 18pt**（`tray-icon` 的内部实现，
//!   见 `set_icon_for_ns_status_item_button`），所以 `icons/tray.png` 用 44×44 px，
//!   正好是 18pt 的 2x 位图，Retina 下才清晰。给 512 的图不会更大，只会让
//!   二进制变大（图标按原始像素编进可执行文件）。
//! - 必须是「模板图」：纯黑 + alpha，由 macOS 自己反色适配浅色/深色菜单栏。
//!   所以这里必须开 `icon_as_template(true)`，否则深色菜单栏下会看不见。
//! - 图形要撑满画布，只留 2px 安全边：整张图的高度都会映射成 18pt，
//!   四周留白越多，实际图形越小。

use tauri::{
    App, AppHandle, Manager, Runtime,
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

/// 托盘图标的 id，方便以后用 `app.tray_by_id` 取到它。
const TRAY_ID: &str = "main";

/// 自定义菜单项的 id。保持英文、不要本地化，日志和测试会引用它。
const MENU_SHOW_WINDOW: &str = "tray-show-window";

/// 创建菜单栏图标：左键点击弹出菜单。
pub fn init(app: &App) -> tauri::Result<()> {
    let show_window = MenuItem::with_id(app, MENU_SHOW_WINDOW, "显示 DevScope", true, None::<&str>)?;
    let about = PredefinedMenuItem::about(app, Some("关于 DevScope"), Some(about_metadata(app)))?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出 DevScope"))?;
    let menu = Menu::with_items(app, &[&show_window, &about, &separator, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true)
        .tooltip("DevScope")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            // 只有「显示 DevScope」需要自己处理；关于/退出是系统预定义项。
            let id: &str = event.id().as_ref();
            if id == MENU_SHOW_WINDOW {
                show_main_window(app);
            }
        })
        .build(app)?;

    Ok(())
}

/// 「关于」面板用的元数据。
///
/// 刻意不设 `icon`：留空时 macOS 直接用 App bundle 里的 `icon.icns`，
/// 和 Dock 看到的图标完全一致；一旦显式传 `Image`，`muda` 不会缩放，
/// 会把图片的像素尺寸当成 pt 尺寸，1024 的图就显示成 1024pt。
fn about_metadata(app: &App) -> AboutMetadata<'static> {
    let package = app.package_info();
    AboutMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        ..Default::default()
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}
