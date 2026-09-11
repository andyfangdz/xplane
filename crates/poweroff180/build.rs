use std::{collections::HashSet, env, fmt::Write, fs, path::PathBuf};

fn main() {
    let directory = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=parameters.csv");
    let parameters = fs::read_to_string("parameters.csv").expect("parameter schema");
    fs::write(
        directory.join("parameters.rs"),
        generate_parameters(&parameters),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=snapshot.csv");
    let snapshot = fs::read_to_string("snapshot.csv").expect("snapshot schema");
    fs::write(directory.join("protocol.rs"), generate_protocol(&snapshot)).unwrap();
}

fn generate_parameters(csv: &str) -> String {
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
    output
}

fn generate_protocol(csv: &str) -> String {
    let mut rows = csv.lines();
    assert_eq!(rows.next(), Some("field,dataref"));
    let fields: Vec<_> = rows
        .map(|row| row.split_once(',').expect("snapshot field and dataref"))
        .collect();
    assert!(!fields.is_empty(), "empty snapshot schema");
    let mut seen = HashSet::new();
    let mut output = format!(
        "pub const LENGTH: usize = {};\npub mod field {{\n",
        fields.len()
    );
    for (index, (name, _)) in fields.iter().enumerate() {
        assert!(
            name.starts_with(|c: char| c.is_ascii_lowercase())
                && name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'),
            "invalid snapshot field: {name}"
        );
        assert!(seen.insert(name), "duplicate snapshot field: {name}");
        writeln!(
            output,
            "pub const {}: usize = {index};",
            name.to_ascii_uppercase()
        )
        .unwrap();
    }
    output.push_str("}\n");
    let header = fields
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(",");
    writeln!(output, "pub const HEADER: &str = {header:?};").unwrap();

    // The runtime samples SDK inputs into the leading slots in this exact order.
    // Computed telemetry follows them, so a source after an empty row is invalid.
    let source_count = fields
        .iter()
        .take_while(|(_, source)| !source.is_empty())
        .count();
    assert!(
        fields[source_count..]
            .iter()
            .all(|(_, source)| source.is_empty()),
        "snapshot datarefs must precede computed fields"
    );
    let sources: Vec<_> = fields[..source_count]
        .iter()
        .map(|(_, source)| *source)
        .collect();
    writeln!(
        output,
        "pub const NAMES: [&str; {source_count}] = {sources:?};"
    )
    .unwrap();
    output
}
