#[macro_export]
macro_rules! ui_println {
    () => {
        $crate::events::message("")?
    };
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::events::emit($crate::events::AppEvent::Message(msg))?
    }};
}
