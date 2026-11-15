//! Gino core: document model, text layout, scene building and rendering.
//!
//! The core lays out every glyph once. Both renderers ([`render_svg`] and
//! [`render_pdf`]) consume only the [`scene`], so what the canvas shows is,
//! glyph for glyph, what the PDF contains.

pub mod compose;
pub mod edit;
pub mod error;
pub mod export;
pub mod fonts;
pub mod guides;
pub mod hit;
pub mod images;
pub mod impose;
pub mod layout;
pub mod model;
pub mod presets;
pub mod render_pdf;
pub mod render_svg;
pub mod scene;
pub mod session;
pub mod snap;
pub mod spike;
pub mod spreads;
pub mod text;
pub mod units;
pub mod zine;

pub use error::Error;
