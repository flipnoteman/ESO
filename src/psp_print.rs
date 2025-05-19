use core::fmt::{self, write, Write};
use psp::sys;

use crate::psp_geometry::Vertex;

const BUF_SIZE: usize = 256;

struct GuBuf {
    data: [u8; BUF_SIZE + 1],   // +1 for NUL
    pos:  usize,
}

impl GuBuf {
    #[inline] fn new() -> Self { Self { data: [0; BUF_SIZE + 1], pos: 0 } }
}

impl Write for GuBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if self.pos == BUF_SIZE { return Err(fmt::Error); }   // truncated
            self.data[self.pos] = b;
            self.pos += 1;
        }
        Ok(())
    }
}

#[inline]
pub fn gu_print_inner(x: i32, y: i32, col: u32, args: fmt::Arguments) {
    // build the C-string
    let mut buf = GuBuf::new();
    let _ = buf.write_fmt(args);
    buf.data[buf.pos] = 0;

    unsafe {
        sys::sceGuDebugPrint(x, y, col, buf.data.as_ptr() as *const u8);
    }
}



#[macro_export]
macro_rules! print_at {
    ($x:expr, $y:expr, $col:expr, $($arg:tt)*) => {{
        $crate::psp_print::gu_print_inner(
            $x as i32, $y as i32, $col as u32,
            core::format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! println_at {
    ($x:expr, $y:expr, $col:expr)                 => { $crate::print_at!($x,$y,$col,"\n") };
    ($x:expr, $y:expr, $col:expr, $fmt:expr)      => { $crate::print_at!($x,$y,$col, concat!($fmt, "\n")) };
    ($x:expr, $y:expr, $col:expr, $fmt:expr $(, $arg:tt)*) =>
        { $crate::print_at!($x,$y,$col, concat!($fmt, "\n"), $($arg)* ) };
}

// default (0,0) white versions
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::print_at!(0, 0, 0xFFFFFFFFu32, $($arg)*); };
}

#[macro_export]
macro_rules! println {
    ()                     => { $crate::print!("\n") };
    ($fmt:expr)            => { $crate::print!(concat!($fmt, "\n")) };
    ($fmt:expr, $($arg:tt)*) =>
        { $crate::print!(concat!($fmt, "\n"), $($arg)* ); };
}
