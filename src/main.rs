// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use native_dialog::FileDialog;
use std::{
    error::Error,
    ffi::OsString,
    fmt,
    fs::{read_dir, DirEntry, File, ReadDir},
    io::{BufReader, BufWriter},
    path::Path,
};
slint::include_modules!();
use image::{codecs::jpeg::JpegEncoder, ColorType, DynamicImage, ImageReader};
use slint::{Color, ComponentHandle, RgbaColor, Weak};

#[derive(Debug)]
enum ConvertToJpegError {
    InputDirectoryInvalid(std::io::Error),
}

impl fmt::Display for ConvertToJpegError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConvertToJpegError::InputDirectoryInvalid(err) => write!(f, "Directory Error: {}", err),
        }
    }
}

impl std::error::Error for ConvertToJpegError {}

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;
    let ui_handle: Weak<AppWindow> = ui.as_weak();

    let dir_select_ui_handle = ui_handle.clone();
    ui.on_select_dir(move |target_text: i32| {
        let ui: AppWindow = dir_select_ui_handle.unwrap();

        ui.set_status_text("Selecting input and output directories".into());
        ui.set_status_text_color(Color::from(RgbaColor {
            red: 1.,
            green: 0.64,
            blue: 0.,
            alpha: 1.,
        }));

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

    let jpeg_ui_handle = ui_handle.clone();
    ui.on_to_jpeg(
        move |inp_dir: slint::SharedString, out_dir: slint::SharedString| {
            let ui: AppWindow = jpeg_ui_handle.unwrap();
            // NOTE: Debugging print statements
            println!("Input Directory: {}", inp_dir);
            println!("Output Directory: {}", out_dir);

            ui.set_status_text("Converting images in progress".into());
            ui.set_status_text_color(Color::from(RgbaColor {
                red: 1.,
                green: 0.,
                blue: 0.,
                alpha: 1.,
            }));

            let paths = match get_files_paths(inp_dir.to_string()) {
                Ok(paths) => paths,
                Err(_) => {
                    // TODO: Add gui output for the io error
                    return;
                }
            };
            let total_files = paths.count() as f32;

            // TODO: Remove the redeclaration of paths by converting it
            // into a vector of type DirEntry
            let paths = match get_files_paths(inp_dir.to_string()) {
                Ok(paths) => paths,
                Err(_) => {
                    // TODO: Add gui output for the io error
                    return;
                }
            };

            let prog_bar_handle = jpeg_ui_handle.clone();
            let _img_conv_thread = std::thread::spawn(move || {
                let completion_msg: &str = "Image conversion complete";
                let mut progress: f32 = 0.0;
                for result in paths {
                    let dir_entry = match result {
                        Ok(dir) => dir,
                        Err(err) => {
                            // TODO: Add logging for each dir entry error
                            println!("Error with a dir entry: {}", err);
                            return;
                        }
                    };
                    convert_img_to_jpeg(dir_entry, out_dir.to_string());
                    progress += 1.0;
                    let completion = progress / total_files;
                    let ui_copy = prog_bar_handle.clone();
                    slint::invoke_from_event_loop(move || {
                        let ui = ui_copy.unwrap();
                        ui.set_conv_progress(completion);
                        if completion >= 1.0 {
                            ui.set_status_text(completion_msg.into());
                            ui.set_status_text_color(Color::from(RgbaColor {
                                red: 0.5,
                                green: 0.65,
                                blue: 0.32,
                                alpha: 1.,
                            }));
                        }
                    });
                }
            });
        },
    );
    ui.run()?;

    Ok(())
}

fn get_files_paths(dir: String) -> Result<ReadDir, crate::ConvertToJpegError> {
    let paths = match read_dir(dir) {
        Ok(paths) => paths,
        Err(err) => {
            // TODO: Log and display error that there
            // was an error with the input directory
            println!(
                "Error with reading the dir while getting file paths: {}",
                err
            );
            return Err(ConvertToJpegError::InputDirectoryInvalid(err));
        }
    };

    Ok(paths)
}

fn convert_img_to_jpeg(dir_entry: DirEntry, out_dir: String) {
    let path_str = match dir_entry.path().into_os_string().into_string() {
        Ok(path_str) => path_str,
        Err(_) => {
            println!("Error while converting dir entry into a string");
            return;
        }
    };
    // Ceate an image decoder from the file opened at path_str
    let img_reader = match ImageReader::open(&path_str.to_string()) {
        Ok(res) => res,
        Err(err) => {
            // TODO: Add logging for ImageReader errors
            println!("Error while opening and an ImageReader: {}", err);
            return;
        }
    };

    // WARNING: This covers the case where the img file extension
    // does not match the file type
    let img_reader: ImageReader<BufReader<File>> = match img_reader.with_guessed_format() {
        Ok(img_reader) => img_reader,
        Err(_) => return,
    };

    // Decode img from image_reader
    let img: DynamicImage = match img_reader.decode() {
        Ok(res) => res.into(),
        Err(err) => {
            // TODO: Add logging for image decoding errors
            println!("Error while decoding to DynamicImage: {}", err);
            println!("  Current file: {}", path_str);
            return;
        }
    };

    let img = img.into_rgb8();

    // Change the file extension to .jpeg
    let out_file = dir_entry.file_name();
    let mut out_file: OsString = match Path::new(&out_file).file_stem() {
        Some(out_file) => out_file.into(),
        None => {
            println!("Error while extracting the file_stem()");
            return;
        }
    };
    out_file.push(".jpg");

    // Combine the output directory with the new filename
    let out_dir = Path::new(&out_dir);
    let out_path = out_dir.join(out_file);

    let out_path: String = match out_path.into_os_string().into_string() {
        Ok(out_path) => out_path,
        Err(_) => {
            println!("Could not convert os_string to string");
            return;
        }
    };

    // Create file to write to
    let out_file = match File::create(out_path) {
        Ok(out_file) => out_file,
        Err(err) => {
            println!("Error while creating output file: {}", err);
            // TODO: Handle file creation errors
            return;
        }
    };

    // Create a BufWriter
    let ref mut file_writer = BufWriter::new(out_file);

    // Create a JPEG encoder
    let mut img_encoder = JpegEncoder::new(file_writer);
    // Encode image to JPEG and handle encode errors
    match img_encoder.encode(
        &img,
        img.dimensions().0,
        img.dimensions().1,
        ColorType::Rgb8.into(),
    ) {
        Ok(()) => (),
        Err(err) => {
            println!("Error while encoding image to jpeg: {}", err);
        }
    };
}
