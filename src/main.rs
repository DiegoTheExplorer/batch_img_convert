// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use native_dialog::FileDialog;
use std::{
    error::Error,
    f32,
    ffi::OsString,
    fmt,
    fs::{read_dir, DirEntry, File},
    io::{BufReader, BufWriter},
    path::Path,
    sync::{Arc, Mutex, Weak},
    thread,
};
slint::include_modules!();
use image::{codecs::jpeg::JpegEncoder, ColorType, DynamicImage, ImageReader};
use slint::ComponentHandle;

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
    // HACK:
    // I'm not sure if this is best practice to be
    // able to reference ui multiple times
    let ui = Arc::new(AppWindow::new()?);
    let ui_handle: Weak<AppWindow> = Arc::downgrade(&ui);

    let ui_handle_dir = ui_handle.clone();
    ui.on_select_dir(move |target_text: i32| {
        let ui: Arc<AppWindow> = match ui_handle_dir.upgrade() {
            Some(ui) => ui,
            None => return,
        };

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

    let ui_handle_jpeg = ui_handle.clone();
    ui.on_to_jpeg(
        move |inp_dir: slint::SharedString, out_dir: slint::SharedString| {
            let ui: Arc<AppWindow> = match ui_handle_jpeg.upgrade() {
                Some(ui) => ui,
                None => return,
            };
            println!("Input Directory: {}", inp_dir);
            println!("Output Directory: {}", out_dir);

            thread::spawn(move || {
                let paths = match get_files_paths(inp_dir.to_string()) {
                    Ok(paths) => paths,
                    Err(_) => {
                        // TODO: Add gui output for the io error
                        return;
                    }
                };

                convert_imgs_to_jpeg(paths, out_dir.to_string(), ui);
            });
        },
    );
    ui.run()?;

    Ok(())
}

fn get_files_paths(dir: String) -> Result<Vec<DirEntry>, crate::ConvertToJpegError> {
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

    let mut path_vec: Vec<DirEntry> = Vec::new();
    for dir_entry in paths {
        match dir_entry {
            Ok(dir_entry) => path_vec.push(dir_entry),
            Err(err) => eprint!("Error pushing to dir_entry vector: {}", err),
        };
    }

    Ok(path_vec)
}

fn convert_imgs_to_jpeg(paths: Vec<DirEntry>, out_dir: String, ui: Arc<AppWindow>) {
    let mut prog_counter: f32 = 0.0;
    let dir_entry_count: f32 = paths.len() as f32;
    let mut prog_percent: f32;

    for dir_entry in paths {
        /*
        let dir_entry = match result {
            Ok(dir) => dir,
            Err(err) => {
                // TODO: Add logging for each dir entry error
                println!("Error with a dir entry: {}", err);
                continue;
            }
        };
        */

        let path_str = match dir_entry.path().into_os_string().into_string() {
            Ok(path_str) => path_str,
            Err(_) => {
                println!("Error while converting dir entry into a string");
                continue;
            }
        };

        // Create an image decoder from the file opened at path_str
        let img_reader = match ImageReader::open(path_str.clone()) {
            Ok(res) => res,
            Err(err) => {
                // TODO: Add logging for ImageReader errors
                println!("Error while opening and an ImageReader: {}", err);
                continue;
            }
        };

        // WARNING: This covers the case where the img file extension
        // does not match the file type
        let img_reader: ImageReader<BufReader<File>> = match img_reader.with_guessed_format() {
            Ok(img_reader) => img_reader,
            Err(_) => continue,
        };

        // Decode img from image_reader
        let img: DynamicImage = match img_reader.decode() {
            Ok(res) => res.into(),
            Err(err) => {
                // TODO: Add logging for image decoding errors
                println!("Error while decoding to DynamicImage: {}", err);
                println!("  Current file: {}", path_str);
                continue;
            }
        };

        let img = img.into_rgb8();

        // Change the file extension to .jpeg
        let out_file = dir_entry.file_name();
        let mut out_file: OsString = match Path::new(&out_file).file_stem() {
            Some(out_file) => out_file.into(),
            None => {
                println!("Error while extracting the file_stem()");
                continue;
            }
        };
        out_file.push(".jpeg");

        // Combine the output directory with the new filename
        let out_dir = Path::new(&out_dir);
        let out_path = out_dir.join(out_file);

        let out_path: String = match out_path.into_os_string().into_string() {
            Ok(out_path) => out_path,
            Err(_) => {
                println!("Could not convert os_string to string");
                continue;
            }
        };

        let out_file = match File::create(out_path) {
            Ok(out_file) => out_file,
            Err(err) => {
                println!("Error while creating output file: {}", err);
                // TODO: Handle file creation errors
                continue;
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
                continue;
            }
        };

        prog_counter = prog_counter + 1.0;
        prog_percent = prog_counter / dir_entry_count;
    }
}
