#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod vga;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

/// Entry point
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    for i in 0..40 {
        println!("line {}", i);
    }

    loop {}
}
