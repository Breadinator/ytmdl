#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod gui;
pub mod parsing;
pub mod playlist;
pub mod scraping;
pub mod utils;

mod download;
pub use download::*;

mod threading;
use threading::POOL;
