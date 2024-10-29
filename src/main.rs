// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use native_dialog::FileDialog;
use std::error::Error;
use std::fs;

slint::include_modules!();
use slint::{ComponentHandle, SharedString, Weak};

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;
    let ui_handle: Weak<AppWindow> = ui.as_weak();

    ui.on_select_dir(move |target_text: i32| {
        let ui: AppWindow = ui_handle.unwrap();

        let path = FileDialog::new()
            .set_location("~")
            .show_open_single_dir()
            .unwrap();

        let path = match path {
            Some(path) => path,
            None => return,
            // TODO: Add error handling for when the File Dialog fails
        };

        let path_str = path.into_os_string().into_string().unwrap().clone();
        match target_text {
            0 => ui.set_inp_dir(path_str.into()),
            1 => ui.set_out_dir(path_str.into()),
            _ => return,
        }
    });

    ui.on_to_jpeg(
        move |inp_dir: slint::SharedString, out_dir: slint::SharedString| {
            println!("Input Directory: {}", inp_dir);
            println!("Output Directory: {}", out_dir);

            get_files_paths(inp_dir.to_string());
        },
    );
    ui.run()?;

    Ok(())
}

fn get_files_paths(dir: String) {
    let paths = fs::read_dir(dir).unwrap();

    for path in paths {
        println!("{}", path.unwrap().path().display());
    }
}
