use iced::{Application, Settings};
use ytmdl::*;

fn main() -> iced::Result {
    env_logger::init();

    gui::App::run(Settings {
        window: iced::window::Settings {
            size: (800, 640),
            ..Default::default()
        },
        ..Default::default()
    })
}
