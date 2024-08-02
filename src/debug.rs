// Writes to `console.log` or stderr (depending on target architecture), using
// the same arguments as [`format!`].
macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug")] {
            let formatted_message = format!($($arg)*);
            #[cfg(target_arch = "wasm32")]
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&formatted_message));
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{}", formatted_message);
        }
    };
}
pub(crate) use debug_log;
