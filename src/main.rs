#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if let Err(error) = personal_downloaders::gui::run(personal_downloaders::Site::Cum) {
        eprintln!("No se pudo abrir Cum.st DL: {error}");
        std::process::exit(1);
    }
}
