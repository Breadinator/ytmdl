use iced::{Application, Settings};
use std::env;
use ytmdl::*;

fn main() -> iced::Result {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "ytmdl");
    }
    if env::var("YTMDL_OUT_DIR").is_err() {
        if let Some(mut p) = dirs::download_dir() {
            p.push("ytmdl");
            env::set_var("YTMDL_OUT_DIR", p);
        }
    }

    env_logger::init();

    gui::App::run(Settings {
        window: iced::window::Settings {
            size: (800, 640),
            ..Default::default()
        },
        ..Default::default()
    })
}
