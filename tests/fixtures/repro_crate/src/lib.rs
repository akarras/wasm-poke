use smallvec::SmallVec;

mod other;

#[no_mangle]
pub extern "C" fn small_vec_test() {
    let mut v: SmallVec<[(f32, f32); 8]> = SmallVec::new();
    v.push((1.0, 2.0));
    v.push((3.0, 4.0));
    other::do_something(&v);
    // testing what it looks like to send a vec into a function
    other::take_small_vec(v);
}

#[no_mangle]
pub extern "C" fn vec_test() {
    let mut v = Vec::with_capacity(8);
    v.push((1.0, 2.0));
    v.push((3.0, 4.0));
    other::do_something(&v);
    other::take_vec(v);
}
