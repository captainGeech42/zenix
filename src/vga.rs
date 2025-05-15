//! Goals:
//!   - scrolling text
//!     - 0,0 is the top left corner. text starts there and moves down
//!     - as you hit the bottom, the line buffer shifts
//!         - (like how xnu or linux scrolls on verbose boots)
//!   - colors
//!   - dlog, ilog, wlog, elog macros
//!     - some way to conditionally enable/disable debug logging, or change the log level
//!
//! links:
//! - reference post (lots of the boilerplate is from here): <https://os.phil-opp.com/vga-text-mode/>
//! - vga symbols: <https://en.wikipedia.org/wiki/Code_page_437>
//! - vga text mode: <https://en.wikipedia.org/wiki/VGA_text_mode>
//! - freevga: <http://www.osdever.net/FreeVGA/home.htm>
//!

use core::fmt;

use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;
use x86_64::structures::port::{PortRead as _, PortWrite as _};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    current_col: usize,
    current_row: usize,
    default_color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.current_col >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = self.current_row;
                let col = self.current_col;

                let color_code = self.default_color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.current_col += 1;
            }
        }
    }

    fn new_line(&mut self) {
        // check if we still have more screen real estate to use
        if self.current_row == BUFFER_HEIGHT - 1 {
            // we ran out of space, shift all the rows up in preparation to overwrite the bottom row
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    let character = self.buffer.chars[row][col].read();
                    self.buffer.chars[row - 1][col].write(character);
                }
            }
            self.clear_row(self.current_row);
        } else {
            // we still have more rows available
            self.current_row += 1;
        }

        self.current_col = 0;
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of printable ASCII range
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.default_color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        current_col: 0,
        current_row: 0,
        default_color_code: ColorCode::new(Color::White, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

/// Write text to the console
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

/// Write a line of text to the console
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}

pub fn disable_cursor() {
    // first, figure out the I/OAS status
    // http://www.osdever.net/FreeVGA/vga/extreg.htm#3CCR3C2W
    let misc_out: u8;
    unsafe {
        misc_out = u8::read_from_port(0x3cc);
    }

    // determine the port addresses based on the lowest bit of the above port read
    // http://www.osdever.net/FreeVGA/vga/crtcreg.htm
    let crtc_addr: u16;
    let crtc_data: u16;
    if (misc_out & 1) == 0 {
        crtc_addr = 0x3b4;
        crtc_data = 0x3b5;
    } else {
        crtc_addr = 0x3d4;
        crtc_data = 0x3d5;
    }

    unsafe {
        // set the address to the Cursor Start Register
        // http://www.osdever.net/FreeVGA/vga/crtcreg.htm#0A
        u8::write_to_port(crtc_addr, 0xa);

        // then, turn it off
        // only bit 4 needs to be set
        u8::write_to_port(crtc_data, 1 << 4);
    }
}
