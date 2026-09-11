use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=parameters.csv");
    let csv = fs::read_to_string("parameters.csv").expect("parameter schema");
    let mut output = String::from("parameters! {\n");
    for row in csv.lines().skip(1) {
        let fields: Vec<_> = row.split(',').collect();
        assert_eq!(fields.len(), 4, "invalid schema row: {row}");
        let values: Vec<f64> = fields[1..].iter().map(|v| v.parse().unwrap()).collect();
        assert!(values.iter().all(|v| v.is_finite()));
        assert!(values[1] <= values[0] && values[0] <= values[2]);
        output.push_str(&format!(
            "{}: ({:?}, {:?}, {:?}),\n",
            fields[0], values[0], values[1], values[2]
        ));
    }
    output.push_str("}\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("parameters.rs"),
        output,
    )
    .unwrap();
}
