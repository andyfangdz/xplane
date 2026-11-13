#[path = "../tests/support/programs.rs"]
mod programs;
fn main() {
    let programs = programs::programs();
    let slices: Vec<_> = programs.iter().map(Vec::as_slice).collect();
    let generated = xplane_rpn::aot::generate(&slices, "xplane_rpn").unwrap();
    print!("{}", generated.rust);
}
