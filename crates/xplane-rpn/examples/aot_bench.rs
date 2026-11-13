use std::{hint::black_box, time::Instant};
use xplane_rpn::*;
#[path = "../tests/support/programs.rs"]
mod programs;
#[allow(dead_code)]
#[rustfmt::skip]
#[path="../tests/fixtures/aot_generated.rs"] mod compiled;
struct Reads {
    value: f64,
    count: u64,
}
impl Host for Reads {
    type Error = ();
    fn get_local(&mut self, _: u32) -> Result<f64, ()> {
        self.count += 1;
        Ok(black_box(self.value))
    }
    fn set_local(&mut self, _: u32, _: &Value) -> Result<(), ()> {
        Ok(())
    }
    fn get_external(&mut self, _: u32) -> Result<Value, ()> {
        self.count += 1;
        Ok(Value::Number(black_box(self.value)))
    }
    fn set_external(&mut self, _: u32, _: &Value, _: bool) -> Result<(), ()> {
        Ok(())
    }
}
fn main() {
    let numeric = std::env::args().any(|a| a == "--numeric");
    let programs = programs::programs();
    let slices: Vec<_> = programs.iter().map(Vec::as_slice).collect();
    let generated = aot::generate(&slices, "xplane_rpn").unwrap();
    let mut code = vec![];
    let mut descriptors = vec![];
    for p in &programs {
        descriptors.push(Program {
            first: code.len() as u32,
            count: p.len() as u32,
        });
        code.extend_from_slice(p);
    }
    let mut vm = Vm::new(black_box(&code), &descriptors).unwrap();
    let mut native = Vm::new(black_box(&code), &descriptors).unwrap();
    for (name, index) in [
        ("arithmetic", 1),
        ("register_stack", 3),
        ("long_numeric", programs.len() - 1),
    ] {
        let id = black_box(generated.program_ids[index]);
        let binding = native
            .bind_aot(index as u32, compiled::shape(id).unwrap())
            .unwrap();
        let mut host = Reads {
            value: black_box(0.75),
            count: 0,
        };
        let mut times = [0.; 4];
        for (repeat, aot) in [false, true, true, false].into_iter().enumerate() {
            for _ in 0..30000 {
                if aot {
                    native.begin(true);
                    // SAFETY: id selects the exact generated shape bound above;
                    // the VM guards forward-only flow and sufficient budget.
                    black_box(
                        unsafe {
                            if numeric {
                                native.evaluate_numeric(&binding, 1000, &mut host, |f| {
                                    compiled::execute_numeric(id, f)
                                })
                            } else {
                                native.evaluate_aot_prevalidated(&binding, 1000, &mut host, |f| {
                                    compiled::execute_prevalidated(id, f)
                                })
                            }
                        }
                        .unwrap(),
                    );
                } else {
                    vm.begin(true);
                    black_box(vm.evaluate(index as u32, 1000, &mut host).unwrap());
                }
            }
            let before = host.count;
            let start = Instant::now();
            for _ in 0..500000 {
                if aot {
                    native.begin(true);
                    // SAFETY: id selects the exact generated shape bound above;
                    // the VM guards forward-only flow and sufficient budget.
                    black_box(
                        unsafe {
                            if numeric {
                                native.evaluate_numeric(&binding, 1000, &mut host, |f| {
                                    compiled::execute_numeric(id, f)
                                })
                            } else {
                                native.evaluate_aot_prevalidated(&binding, 1000, &mut host, |f| {
                                    compiled::execute_prevalidated(id, f)
                                })
                            }
                        }
                        .unwrap(),
                    );
                } else {
                    vm.begin(true);
                    black_box(vm.evaluate(index as u32, 1000, &mut host).unwrap());
                }
            }
            times[repeat] = start.elapsed().as_secs_f64() * 1e9 / 500000.;
            assert_eq!(
                host.count - before,
                500000,
                "every execution must keep its external read"
            );
        }
        let vm_ns = (times[0] + times[3]) / 2.;
        let aot_ns = (times[1] + times[2]) / 2.;
        println!(
            "{name}: vm_ns={vm_ns:.3} aot_ns={aot_ns:.3} reduction_pct={:.3} samples={times:?}",
            100. * (vm_ns - aot_ns) / vm_ns
        );
    }
}
