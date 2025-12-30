//! macOS menu bar implementation
//!
//! Creates and manages the native macOS menu bar with standard terminal actions.

#[cfg(target_os = "macos")]
use muda::{
    AboutMetadata, ContextMenu, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers as AccelMods},
};

/// Menu action identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    // Shell menu
    NewTab,
    NewWindow,
    RenameWindow,
    CloseTab,
    CloseWindow,
    Quit,
    // Edit menu
    Copy,
    Paste,
    SelectAll,
    Find,
    ClearScrollback,
    // View menu
    ToggleFullScreen,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ToggleProfiling,
    // Window menu
    Minimize,
    NextTab,
    PrevTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectTab9,
}

/// Menu item IDs stored for event handling
#[cfg(target_os = "macos")]
pub struct MenuIds {
    pub new_tab: MenuId,
    pub new_window: MenuId,
    pub rename_window: MenuId,
    pub close_tab: MenuId,
    pub close_window: MenuId,
    pub quit: MenuId,
    pub copy: MenuId,
    pub paste: MenuId,
    pub select_all: MenuId,
    pub find: MenuId,
    pub clear_scrollback: MenuId,
    pub toggle_fullscreen: MenuId,
    pub increase_font: MenuId,
    pub decrease_font: MenuId,
    pub reset_font: MenuId,
    pub toggle_profiling: MenuId,
    pub minimize: MenuId,
    pub next_tab: MenuId,
    pub prev_tab: MenuId,
    pub select_tab: [MenuId; 9],
}

#[cfg(target_os = "macos")]
pub fn build_menu_bar() -> (Menu, MenuIds, Submenu) {
    let menu = Menu::new();

    // App menu (CRT)
    let about_metadata = AboutMetadata {
        name: Some("CRT".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    };
    let app_menu = Submenu::with_items(
        "CRT",
        true,
        &[
            &PredefinedMenuItem::about(None, Some(about_metadata)),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
        ],
    )
    .unwrap();

    // Shell menu
    let new_tab = MenuItem::with_id(
        "new_tab",
        "New Tab",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyT)),
    );
    let new_window = MenuItem::with_id(
        "new_window",
        "New Window",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyN)),
    );
    let rename_window = MenuItem::with_id(
        "rename_window",
        "Rename Window...",
        true,
        Some(Accelerator::new(
            Some(AccelMods::SUPER | AccelMods::SHIFT),
            Code::KeyR,
        )),
    );
    let close_tab = MenuItem::with_id(
        "close_tab",
        "Close Tab",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyW)),
    );
    let close_window = MenuItem::with_id(
        "close_window",
        "Close Window",
        true,
        Some(Accelerator::new(
            Some(AccelMods::SUPER | AccelMods::SHIFT),
            Code::KeyW,
        )),
    );
    let quit = MenuItem::with_id(
        "quit",
        "Quit CRT",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyQ)),
    );

    let shell_menu = Submenu::with_items(
        "Shell",
        true,
        &[
            &new_tab,
            &new_window,
            &PredefinedMenuItem::separator(),
            &rename_window,
            &PredefinedMenuItem::separator(),
            &close_tab,
            &close_window,
            &PredefinedMenuItem::separator(),
            &quit,
        ],
    )
    .unwrap();

    // Edit menu
    let copy = MenuItem::with_id(
        "copy",
        "Copy",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyC)),
    );
    let paste = MenuItem::with_id(
        "paste",
        "Paste",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyV)),
    );
    let select_all = MenuItem::with_id(
        "select_all",
        "Select All",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyA)),
    );
    let find = MenuItem::with_id(
        "find",
        "Find...",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyF)),
    );
    let clear_scrollback = MenuItem::with_id(
        "clear_scrollback",
        "Clear Scrollback",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyK)),
    );

    let edit_menu = Submenu::with_items(
        "Edit",
        true,
        &[
            &copy,
            &paste,
            &select_all,
            &PredefinedMenuItem::separator(),
            &find,
            &PredefinedMenuItem::separator(),
            &clear_scrollback,
        ],
    )
    .unwrap();

    // View menu
    let toggle_fullscreen = MenuItem::with_id(
        "toggle_fullscreen",
        "Enter Full Screen",
        true,
        Some(Accelerator::new(
            Some(AccelMods::SUPER | AccelMods::CONTROL),
            Code::KeyF,
        )),
    );
    let increase_font = MenuItem::with_id(
        "increase_font",
        "Increase Font Size",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Equal)),
    );
    let decrease_font = MenuItem::with_id(
        "decrease_font",
        "Decrease Font Size",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Minus)),
    );
    let reset_font = MenuItem::with_id(
        "reset_font",
        "Reset Font Size",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit0)),
    );
    let toggle_profiling = MenuItem::with_id(
        "toggle_profiling",
        "Start Profiling",
        true,
        Some(Accelerator::new(
            Some(AccelMods::SUPER | AccelMods::ALT),
            Code::KeyP,
        )),
    );

    let view_menu = Submenu::with_items(
        "View",
        true,
        &[
            &toggle_fullscreen,
            &PredefinedMenuItem::separator(),
            &increase_font,
            &decrease_font,
            &reset_font,
            &PredefinedMenuItem::separator(),
            &toggle_profiling,
        ],
    )
    .unwrap();

    // Window menu
    let minimize = MenuItem::with_id(
        "minimize",
        "Minimize",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyM)),
    );
    let next_tab = MenuItem::with_id(
        "next_tab",
        "Show Next Tab",
        true,
        Some(Accelerator::new(
            Some(AccelMods::SUPER | AccelMods::SHIFT),
            Code::BracketRight,
        )),
    );
    let prev_tab = MenuItem::with_id(
        "prev_tab",
        "Show Previous Tab",
        true,
        Some(Accelerator::new(
            Some(AccelMods::SUPER | AccelMods::SHIFT),
            Code::BracketLeft,
        )),
    );

    // Tab selection items
    let select_tab_1 = MenuItem::with_id(
        "select_tab_1",
        "Select Tab 1",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit1)),
    );
    let select_tab_2 = MenuItem::with_id(
        "select_tab_2",
        "Select Tab 2",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit2)),
    );
    let select_tab_3 = MenuItem::with_id(
        "select_tab_3",
        "Select Tab 3",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit3)),
    );
    let select_tab_4 = MenuItem::with_id(
        "select_tab_4",
        "Select Tab 4",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit4)),
    );
    let select_tab_5 = MenuItem::with_id(
        "select_tab_5",
        "Select Tab 5",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit5)),
    );
    let select_tab_6 = MenuItem::with_id(
        "select_tab_6",
        "Select Tab 6",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit6)),
    );
    let select_tab_7 = MenuItem::with_id(
        "select_tab_7",
        "Select Tab 7",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit7)),
    );
    let select_tab_8 = MenuItem::with_id(
        "select_tab_8",
        "Select Tab 8",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit8)),
    );
    let select_tab_9 = MenuItem::with_id(
        "select_tab_9",
        "Select Tab 9",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit9)),
    );

    let window_menu = Submenu::with_items(
        "Window",
        true,
        &[
            &minimize,
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::bring_all_to_front(None),
            &PredefinedMenuItem::separator(),
            &next_tab,
            &prev_tab,
            &PredefinedMenuItem::separator(),
            &select_tab_1,
            &select_tab_2,
            &select_tab_3,
            &select_tab_4,
            &select_tab_5,
            &select_tab_6,
            &select_tab_7,
            &select_tab_8,
            &select_tab_9,
        ],
    )
    .unwrap();

    // Build the menu bar
    menu.append(&app_menu).unwrap();
    menu.append(&shell_menu).unwrap();
    menu.append(&edit_menu).unwrap();
    menu.append(&view_menu).unwrap();
    menu.append(&window_menu).unwrap();

    let ids = MenuIds {
        new_tab: new_tab.id().clone(),
        new_window: new_window.id().clone(),
        rename_window: rename_window.id().clone(),
        close_tab: close_tab.id().clone(),
        close_window: close_window.id().clone(),
        quit: quit.id().clone(),
        copy: copy.id().clone(),
        paste: paste.id().clone(),
        select_all: select_all.id().clone(),
        find: find.id().clone(),
        clear_scrollback: clear_scrollback.id().clone(),
        toggle_fullscreen: toggle_fullscreen.id().clone(),
        increase_font: increase_font.id().clone(),
        decrease_font: decrease_font.id().clone(),
        reset_font: reset_font.id().clone(),
        toggle_profiling: toggle_profiling.id().clone(),
        minimize: minimize.id().clone(),
        next_tab: next_tab.id().clone(),
        prev_tab: prev_tab.id().clone(),
        select_tab: [
            select_tab_1.id().clone(),
            select_tab_2.id().clone(),
            select_tab_3.id().clone(),
            select_tab_4.id().clone(),
            select_tab_5.id().clone(),
            select_tab_6.id().clone(),
            select_tab_7.id().clone(),
            select_tab_8.id().clone(),
            select_tab_9.id().clone(),
        ],
    };

    (menu, ids, window_menu)
}

/// Set the Window submenu as the macOS Windows menu
/// This enables automatic window listing in the menu and dock
#[cfg(target_os = "macos")]
pub fn set_windows_menu(window_submenu: &Submenu) {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    // Get NSApp and the NSMenu from the submenu
    let ns_menu_ptr = window_submenu.ns_menu();

    unsafe {
        // Get NSApplication shared instance
        let mtm = MainThreadMarker::new().expect("must be on main thread");
        let app = NSApplication::sharedApplication(mtm);

        // Cast the raw pointer to NSMenu
        let ns_menu: Retained<AnyObject> = Retained::retain(ns_menu_ptr as *mut AnyObject).unwrap();

        // Call setWindowsMenu: on NSApp
        let _: () = msg_send![&app, setWindowsMenu: &*ns_menu];
    }
}

#[cfg(target_os = "macos")]
pub fn menu_id_to_action(id: &MenuId, ids: &MenuIds) -> Option<MenuAction> {
    if *id == ids.new_tab {
        return Some(MenuAction::NewTab);
    }
    if *id == ids.new_window {
        return Some(MenuAction::NewWindow);
    }
    if *id == ids.rename_window {
        return Some(MenuAction::RenameWindow);
    }
    if *id == ids.close_tab {
        return Some(MenuAction::CloseTab);
    }
    if *id == ids.close_window {
        return Some(MenuAction::CloseWindow);
    }
    if *id == ids.quit {
        return Some(MenuAction::Quit);
    }
    if *id == ids.copy {
        return Some(MenuAction::Copy);
    }
    if *id == ids.paste {
        return Some(MenuAction::Paste);
    }
    if *id == ids.select_all {
        return Some(MenuAction::SelectAll);
    }
    if *id == ids.find {
        return Some(MenuAction::Find);
    }
    if *id == ids.clear_scrollback {
        return Some(MenuAction::ClearScrollback);
    }
    if *id == ids.toggle_fullscreen {
        return Some(MenuAction::ToggleFullScreen);
    }
    if *id == ids.increase_font {
        return Some(MenuAction::IncreaseFontSize);
    }
    if *id == ids.decrease_font {
        return Some(MenuAction::DecreaseFontSize);
    }
    if *id == ids.reset_font {
        return Some(MenuAction::ResetFontSize);
    }
    if *id == ids.toggle_profiling {
        return Some(MenuAction::ToggleProfiling);
    }
    if *id == ids.minimize {
        return Some(MenuAction::Minimize);
    }
    if *id == ids.next_tab {
        return Some(MenuAction::NextTab);
    }
    if *id == ids.prev_tab {
        return Some(MenuAction::PrevTab);
    }
    if *id == ids.select_tab[0] {
        return Some(MenuAction::SelectTab1);
    }
    if *id == ids.select_tab[1] {
        return Some(MenuAction::SelectTab2);
    }
    if *id == ids.select_tab[2] {
        return Some(MenuAction::SelectTab3);
    }
    if *id == ids.select_tab[3] {
        return Some(MenuAction::SelectTab4);
    }
    if *id == ids.select_tab[4] {
        return Some(MenuAction::SelectTab5);
    }
    if *id == ids.select_tab[5] {
        return Some(MenuAction::SelectTab6);
    }
    if *id == ids.select_tab[6] {
        return Some(MenuAction::SelectTab7);
    }
    if *id == ids.select_tab[7] {
        return Some(MenuAction::SelectTab8);
    }
    if *id == ids.select_tab[8] {
        return Some(MenuAction::SelectTab9);
    }
    None
}
