#[macro_export]
macro_rules! ui_println {
    () => {
        println!("");
        _ = $crate::events::message("");
    };
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg);
        _ = $crate::events::emit($crate::events::AppEvent::Message(msg));
    }};
}
