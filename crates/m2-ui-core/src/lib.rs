//! Engine-independent core of the script-driven UI (no Bevy, no Python, no GPU).
//!
//! Ports the logic of `neoeter-python/src/bindings/Python{Window,WindowManager,
//! SlotWindow,GridSlotWindow}.cpp` and the layout parts of
//! `neoeter/src/render/{TextTag,GrpTextInstance,...}.cpp`. Where C++ calls a
//! Python handler (`PyCallClassMemberFunc(m_poHandler, "OnX", ...)`) the core
//! emits a [`UiEvent`] through the [`UiHost`] it was given; the host decides
//! whether a handler exists and what it returns.
//!
//! All text APIs are UTF-8 (`&str`, decision D14); legacy code page bytes enter
//! through [`text::TextInstance::set_value_bytes`].

pub mod anim;
pub mod edit;
pub mod event;
pub mod slot;
pub mod text;
pub mod text_tag;
mod textline;
pub mod window;

pub use event::{UiEvent, UiHost};
pub use window::{Ui, WindowId, WindowType};
