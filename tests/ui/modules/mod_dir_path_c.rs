// run-pass
#![allow(unused_macros, non_camel_case_types)]

#[path = "mod_dir_path_c.h"]
pub mod syrup;

pub fn main() {
    let mut x = syrup::foo_t {
        a: 1,
        b: 2,
    };
    unsafe {
        syrup::set_a(&mut x, syrup::foo);
    }
    println!("{} {}", x.a, x.b);
}
