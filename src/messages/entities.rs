//! Entities

pub type MediaId = u64;

pub use serde_types::entities::*;

string_enums! {
    /// Represents the `resize` field in `Size`.
    #[derive(Clone, Debug)]
    pub enum Resize {
        /// The media was resized to fit one dimension, keeping its native aspect ratio.
        :Fit("fit"),
        /// The media was cropped in order to fit a specific resolution.
        :Crop("crop");
        :Custom(_),
    }
}
