//! Document model of LVGLab.
//!
//! The model is a faithful, serializable description of an LVGL 8 user
//! interface. It knows nothing about rendering or code generation; those live
//! in the `lvglab-codegen` crate and the studio application.

#[macro_use]
mod macros;

pub mod catalog;
pub mod color;
pub mod color_theme;
pub mod coord;
pub mod event;
pub mod flags;
pub mod layout;
pub mod project;
pub mod prop;
pub mod series;
pub mod style;
pub mod theme;
pub mod widget;

pub use color::{Color, Palette};
pub use coord::{Align, Coord, Unit};
pub use project::{Project, ProjectError};
pub use widget::{Widget, WidgetId, WidgetKind};

#[doc(hidden)]
pub mod __private {
    pub use serde;
}
