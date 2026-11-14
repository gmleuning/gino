//! Gino's core: document model, text layout, scene building and rendering.
//!
//! The core lays out every glyph once. Both renderers ([`render_svg`] and
//! [`render_pdf`]) consume only the [`scene`], so what the canvas shows is,
//! glyph for glyph, what the PDF contains.

pub mod compose;
pub mod error;
pub mod fonts;
pub mod layout;
pub mod model;
pub mod render_pdf;
pub mod render_svg;
pub mod scene;
pub mod spike;
pub mod units;

pub use error::Error;
