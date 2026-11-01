//! Rust side of the Python 2.7 host.
//!
//! The interpreter stays in C++ (`neoeter-python`); this crate talks to it
//! through `include/Neoeter/Python/Abi/neoeter_python.h`.

use std::ffi::{CString, c_char, c_uint};
use std::fmt;
use std::marker::PhantomData;

mod ffi {
    use super::{c_char, c_uint};

    pub type NeoeterResult = i32;
    pub const NEOETER_OK: NeoeterResult = 0;

    unsafe extern "C" {
        pub fn neoeter_python_version() -> c_uint;
        pub fn neoeter_python_init(program_name: *const c_char) -> NeoeterResult;
        pub fn neoeter_python_run_line(source: *const c_char) -> NeoeterResult;
        pub fn neoeter_python_shutdown();
    }
}

/// Header version this crate was written against, as `(major, minor)`.
pub const ABI_VERSION: (u32, u32) = (0, 1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The library was built from an incompatible header.
    AbiMismatch { found: (u32, u32) },
    /// A string passed in contained a NUL byte.
    InteriorNul,
    /// The host returned a `neoeter_result` other than OK.
    Host(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AbiMismatch { found } => write!(
                f,
                "neoeter_python_abi {}.{} does not match {}.{}",
                found.0, found.1, ABI_VERSION.0, ABI_VERSION.1
            ),
            Error::InteriorNul => write!(f, "string contains a NUL byte"),
            Error::Host(code) => write!(f, "python host returned {code}"),
        }
    }
}

impl std::error::Error for Error {}

/// The running interpreter. One per process, main thread only: it is neither
/// `Send` nor `Sync`, and dropping it runs `Py_Finalize`.
pub struct PythonHost {
    _not_send: PhantomData<*const ()>,
}

impl PythonHost {
    /// Starts the interpreter with the host modules (`dbg`, `time`, `math`).
    pub fn start(program_name: &str) -> Result<Self, Error> {
        let raw = unsafe { ffi::neoeter_python_version() };
        let found = (raw >> 16, raw & 0xffff);
        if found.0 != ABI_VERSION.0 || found.1 < ABI_VERSION.1 {
            return Err(Error::AbiMismatch { found });
        }

        let name = CString::new(program_name).map_err(|_| Error::InteriorNul)?;
        check(unsafe { ffi::neoeter_python_init(name.as_ptr()) })?;
        Ok(Self {
            _not_send: PhantomData,
        })
    }

    /// Runs `source` in `__main__`. A raised exception is logged by the host
    /// and returned as an error.
    pub fn run_line(&self, source: &str) -> Result<(), Error> {
        let source = CString::new(source).map_err(|_| Error::InteriorNul)?;
        check(unsafe { ffi::neoeter_python_run_line(source.as_ptr()) })
    }
}

impl Drop for PythonHost {
    fn drop(&mut self) {
        unsafe { ffi::neoeter_python_shutdown() }
    }
}

fn check(result: ffi::NeoeterResult) -> Result<(), Error> {
    if result == ffi::NEOETER_OK {
        Ok(())
    } else {
        Err(Error::Host(result))
    }
}
