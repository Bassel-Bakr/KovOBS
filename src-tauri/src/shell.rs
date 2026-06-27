#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub trait ShellExt {
    fn no_window(&mut self) -> &mut Self;
}

macro_rules! impl_shell_ext {
    ($ty:ty) => {
        impl ShellExt for $ty {
            fn no_window(&mut self) -> &mut Self {
                #[cfg(windows)]
                self.creation_flags(CREATE_NO_WINDOW);
                
                self
            }
        }
    };
}

impl_shell_ext!(std::process::Command);
impl_shell_ext!(tokio::process::Command);
