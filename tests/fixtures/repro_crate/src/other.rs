use smallvec::SmallVec;

#[inline(never)]
pub fn do_something(v: &[(f32, f32)]) {
    println!("Vector length: {:?}", v);
}

#[inline(always)]
pub fn take_small_vec(v: SmallVec<[(f32, f32); 8]>) {
    println!("v: {:?}", v);
}

#[inline(always)]
pub fn take_vec(v: Vec<(f32, f32)>) {
    println!("v: {v:?}")
}