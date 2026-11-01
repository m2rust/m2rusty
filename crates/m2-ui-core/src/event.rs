//! The seam to the script layer: instead of `PyCallClassMemberFunc` the core
//! hands a [`UiEvent`] to its [`UiHost`].

use crate::text::TextMeasure;
use crate::window::WindowId;

/// A call C++ would make on the window's Python handler object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiEvent {
    /// The window whose handler is called; `None` is the global mouse handler
    /// (`CWindowManager::SetMouseHandler`).
    pub window: Option<WindowId>,
    /// Method name, exactly as the Python scripts define it
    /// (`OnMouseLeftButtonDown`, `CallEvent`, `OnSelectItemSlot`, ...).
    pub name: &'static str,
    /// Integer arguments, in call order.
    pub args: Vec<i64>,
}

/// Implemented by the script bridge (or a test recorder).
///
/// The core never re-enters itself from `call`: a handler that wants to change
/// the UI must queue that change and apply it after the dispatching `Ui`
/// method returns (C++ lets the Python call mutate the live window list; the
/// bridge has to serialise that).
pub trait UiHost: TextMeasure {
    /// Does the handler of `window` (or the mouse handler for `None`) define
    /// `name`? C++ silently does nothing when it does not.
    fn has_handler(&self, window: Option<WindowId>, name: &str) -> bool;

    /// Delivers the event. `Some(v)` is a Python return value that is a
    /// number (C++ reads it as a `long`); `None` is any other return value.
    fn call(&mut self, event: &UiEvent) -> Option<i64>;

    /// Size of an image resource (what `CGraphicImage` reports once loaded);
    /// `None` when it cannot be loaded. Buttons and image boxes size
    /// themselves from it.
    fn image_size(&mut self, _path: &str) -> Option<(i32, i32)> {
        None
    }

    /// `timeGetTime()` in milliseconds.
    fn time_ms(&mut self) -> u32 {
        0
    }

    /// `CTimer::GetCurrentSecond()` (slot cool times).
    fn current_second(&mut self) -> f32 {
        0.0
    }
}
