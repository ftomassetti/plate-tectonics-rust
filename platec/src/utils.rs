//! The assertion macro and shared constants.
//!
//! plate-tectonics, a plate tectonics simulation library
//! Copyright (C) 2012-2013 Lauri Viitanen
//! Copyright (C) 2014-2015 Federico Tomassetti, Bret Curtis
//! Licensed under the GNU LGPL v2.1 or later.

/// The double-precision literal rounded to `f32`, i.e. the
/// `f32`-rounded value of pi. `std::f32::consts::PI` is bit-identical to it.
pub const PI: f32 = std::f32::consts::PI;

/// Assertion that logs rather than aborting.
///
/// Several assertions legitimately fire on paths that then return
/// `BAD_INDEX`, so the
/// behaviour these assertions are *validated against* is log-and-continue —
/// several of them (e.g. in `Rectangle::getMapIndex`) legitimately fire on
/// paths that then return `BAD_INDEX`. We therefore always log rather than
/// abort. Enable the `strict_asserts` feature to make them fatal instead.
#[macro_export]
macro_rules! platec_assert {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            if cfg!(feature = "strict_asserts") {
                panic!(
                    "Assertion `{}` failed in {} line {} Message: {}",
                    stringify!($cond),
                    file!(),
                    line!(),
                    $msg
                );
            } else {
                $crate::utils::log_assert_failure(stringify!($cond), file!(), line!(), $msg);
            }
        }
    };
}

/// Assertion logging.
#[doc(hidden)]
pub fn log_assert_failure(cond: &str, file: &str, line: u32, msg: impl std::fmt::Display) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("Assertion `{cond}` failed in {file} line {line} Message: {msg}");
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (cond, file, line, msg);
    }
}
