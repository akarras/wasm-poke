#![no_std]

// Simple exported functions and generic helpers to encourage monomorphization
// when compiled to wasm32-unknown-unknown.
//
// We keep everything `no_std` and avoid allocation. The generic helpers are
// intentionally marked #[inline(never)] so they show up as distinct function
// bodies in the wasm and don't get inlined away.

// Basic exported function to validate presence in exports.
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[no_mangle]
pub extern "C" fn mul(a: i32, b: i32) -> i32 {
    a * b
}

// A small helper to make sure some work is done in a generic context.
#[inline(never)]
fn generic_sum<T>(a: T, b: T) -> T
where
    T: Copy + core::ops::Add<Output = T>,
{
    a + b
}

// A looped accumulation that generates more code per monomorphization.
#[inline(never)]
fn generic_sum_n<T>(mut acc: T, step: T, n: u32) -> T
where
    T: Copy + core::ops::Add<Output = T>,
{
    let mut i = 0;
    while i < n {
        acc = generic_sum(acc, step);
        i += 1;
    }
    acc
}

// Exported wrappers for different monomorphizations (i32, i64, f32, f64)
#[no_mangle]
pub extern "C" fn call_generic_i32(a: i32, b: i32) -> i32 {
    generic_sum(a, b)
}

#[no_mangle]
pub extern "C" fn call_generic_i64(a: i64, b: i64) -> i64 {
    generic_sum(a, b)
}

#[no_mangle]
pub extern "C" fn call_generic_f32(a: f32, b: f32) -> f32 {
    generic_sum(a, b)
}

#[no_mangle]
pub extern "C" fn call_generic_f64(a: f64, b: f64) -> f64 {
    generic_sum(a, b)
}

// Variants that run the loop to generate larger function bodies per type.
#[no_mangle]
pub extern "C" fn call_generic_i32_n(acc: i32, step: i32, n: u32) -> i32 {
    generic_sum_n(acc, step, n)
}

#[no_mangle]
pub extern "C" fn call_generic_i64_n(acc: i64, step: i64, n: u32) -> i64 {
    generic_sum_n(acc, step, n)
}

#[no_mangle]
pub extern "C" fn call_generic_f32_n(acc: f32, step: f32, n: u32) -> f32 {
    generic_sum_n(acc, step, n)
}

#[no_mangle]
pub extern "C" fn call_generic_f64_n(acc: f64, step: f64, n: u32) -> f64 {
    generic_sum_n(acc, step, n)
}

// A slightly different generic to produce even more distinct monomorphizations.
// This uses a simple "ax + b" computation in a loop to ensure meaningful code size.
#[inline(never)]
fn generic_axpb_n<T>(mut x: T, a: T, b: T, n: u32) -> T
where
    T: Copy + core::ops::Add<Output = T> + core::ops::Mul<Output = T>,
{
    let mut i = 0;
    while i < n {
        x = a * x + b;
        i += 1;
    }
    x
}

#[no_mangle]
pub extern "C" fn call_axpb_i32_n(x: i32, a: i32, b: i32, n: u32) -> i32 {
    generic_axpb_n(x, a, b, n)
}

#[no_mangle]
pub extern "C" fn call_axpb_i64_n(x: i64, a: i64, b: i64, n: u32) -> i64 {
    generic_axpb_n(x, a, b, n)
}

#[no_mangle]
pub extern "C" fn call_axpb_f32_n(x: f32, a: f32, b: f32, n: u32) -> f32 {
    generic_axpb_n(x, a, b, n)
}

#[no_mangle]
pub extern "C" fn call_axpb_f64_n(x: f64, a: f64, b: f64, n: u32) -> f64 {
    generic_axpb_n(x, a, b, n)
}

// Minimal panic handler for no_std wasm cdylib
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
