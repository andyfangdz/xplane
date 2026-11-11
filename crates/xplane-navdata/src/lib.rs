//! Navigation records and ordered layer merging, independent of the simulator.
#![forbid(unsafe_code)]
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub row: i32,
    pub frequency: i32,
    pub service_class: i32,
    pub position: Position,
    pub parameter: f64,
    pub ident: Vec<u8>,
    pub airport: Vec<u8>,
    pub region: Vec<u8>,
    pub runway: Vec<u8>,
    pub name: Vec<u8>,
    pub source: Vec<u8>,
    pub line: u32,
}
impl Record {
    pub fn pair_key(&self) -> Vec<u8> {
        [&self.ident[..], b"|", &self.airport, b"|", &self.region].concat()
    }
    pub fn key(&self) -> Vec<u8> {
        let row = match self.row {
            5 => 4,
            13 => 12,
            value => value,
        };
        let mut out = [row.to_string().as_bytes(), b"|", &self.pair_key()].concat();
        if (4..=9).contains(&self.row) {
            out.push(b'|');
            out.extend_from_slice(&self.runway);
        }
        out
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct File {
    pub version: i32,
    pub header: Vec<u8>,
    pub source: Vec<u8>,
    pub records: Vec<Record>,
    pub ignored_rows: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub source: Vec<u8>,
    pub line: u32,
    pub reason: &'static str,
}
impl Error {
    pub fn message(&self) -> Vec<u8> {
        [
            &self.source[..],
            b":",
            self.line.to_string().as_bytes(),
            b": ",
            self.reason.as_bytes(),
        ]
        .concat()
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.message()))
    }
}
impl std::error::Error for Error {}

/// Numeric extraction consumes a prefix, including its leading whitespace.
/// Conversion may update floating-point status, even when extraction fails.
pub trait Numbers {
    fn integer(&self, input: &[u8]) -> Option<(i32, usize)>;
    fn number(&self, input: &[u8]) -> Option<(f64, usize)>;
}
pub type DuplicateReporter<'a> = &'a mut dyn FnMut(&[u8]);
#[cfg(all(windows, target_env = "msvc"))]
pub struct MsvcNumbers;
#[cfg(all(windows, target_env = "msvc"))]
impl Numbers for MsvcNumbers {
    fn integer(&self, input: &[u8]) -> Option<(i32, usize)> {
        let mut cursor = xplane_numerics::stream::Cursor::new(input);
        cursor.integer().map(|value| (value, cursor.position()))
    }
    fn number(&self, input: &[u8]) -> Option<(f64, usize)> {
        let mut cursor = xplane_numerics::stream::Cursor::new(input);
        cursor.number().map(|value| (value, cursor.position()))
    }
}
fn space(value: u8) -> bool {
    b" \t\r\n\x0b\x0c".contains(&value)
}
struct Fields<'a> {
    input: &'a [u8],
    position: usize,
    numbers: &'a dyn Numbers,
}
impl<'a> Fields<'a> {
    fn integer(&mut self) -> Option<i32> {
        let (value, used) = self.numbers.integer(&self.input[self.position..])?;
        self.position += used;
        Some(value)
    }
    fn number(&mut self) -> Option<f64> {
        let (value, used) = self.numbers.number(&self.input[self.position..])?;
        self.position += used;
        Some(value)
    }
    fn word(&mut self) -> Option<&'a [u8]> {
        while self.input.get(self.position).is_some_and(|v| space(*v)) {
            self.position += 1;
        }
        let first = self.position;
        while self.input.get(self.position).is_some_and(|v| !space(*v)) {
            self.position += 1;
        }
        (self.position != first).then_some(&self.input[first..self.position])
    }
    fn rest(&mut self) -> &'a [u8] {
        while self.input.get(self.position).is_some_and(|v| space(*v)) {
            self.position += 1;
        }
        let mut value = &self.input[self.position..];
        if value.last() == Some(&b'\r') {
            value = &value[..value.len() - 1];
        }
        value
    }
}

/// Parses 1150/1200 radio records. With a duplicate callback, the first identity
/// in a layer is retained and each later occurrence is reported; otherwise a
/// duplicate is an error. Source names and all textual fields remain bytes.
pub fn parse(
    input: &[u8],
    source: &[u8],
    numbers: &dyn Numbers,
    duplicate: Option<DuplicateReporter<'_>>,
) -> Result<File, Error> {
    parse_lines(
        input.split_inclusive(|&v| v == b'\n'),
        source,
        numbers,
        duplicate,
    )
}
/// Streaming form: each item is one line, optionally including its LF. Numeric
/// extraction and duplicate reports happen before requesting the next line.
pub fn parse_lines<I>(
    lines: I,
    source: &[u8],
    numbers: &dyn Numbers,
    mut duplicate: Option<DuplicateReporter<'_>>,
) -> Result<File, Error>
where
    I: IntoIterator,
    I::Item: AsRef<[u8]>,
{
    let mut file = File {
        source: source.to_vec(),
        ..File::default()
    };
    let (mut phase, mut ended, mut line_number) = (0, false, 0u32);
    let mut keys = BTreeSet::new();
    for raw in lines {
        let raw = raw.as_ref();
        line_number = line_number.wrapping_add(1);
        let mut line = raw.strip_suffix(b"\n").unwrap_or(raw);
        if line_number == 1 {
            line = line.strip_prefix(b"\xef\xbb\xbf").unwrap_or(line);
        }
        if line.iter().all(|v| b" \t\r".contains(v)) {
            continue;
        }
        let bad = |reason| Error {
            source: source.to_vec(),
            line: line_number,
            reason,
        };
        if ended {
            return Err(bad("content after navigation terminator"));
        }
        let mut fields = Fields {
            input: line,
            position: 0,
            numbers,
        };
        if phase == 0 {
            let origin = fields.word();
            if !matches!(origin, Some(b"I" | b"A")) || fields.word().is_some() {
                return Err(bad("invalid navigation origin header"));
            }
            phase = 1;
            continue;
        }
        if phase == 1 {
            file.version = fields
                .integer()
                .ok_or_else(|| bad("unsupported navigation data version"))?;
            if fields.word() != Some(b"Version") || !matches!(file.version, 1150 | 1200) {
                return Err(bad("unsupported navigation data version"));
            }
            file.header = line.to_vec();
            phase = 2;
            continue;
        }
        let row = fields.integer().ok_or_else(|| bad("invalid row code"))?;
        if row == 99 {
            if fields.word().is_some() {
                return Err(bad("invalid terminator"));
            }
            ended = true;
            continue;
        }
        if matches!(row, 14..=16) {
            file.ignored_rows = file.ignored_rows.wrapping_add(1);
            continue;
        }
        if !(2..=13).contains(&row) || matches!(row, 10 | 11) {
            return Err(bad("unsupported radio row code"));
        }
        let mut record = Record {
            row,
            source: source.to_vec(),
            line: line_number,
            ..Record::default()
        };
        let incomplete = || bad("incomplete radio record");
        record.position.latitude = fields.number().ok_or_else(incomplete)?;
        record.position.longitude = fields.number().ok_or_else(incomplete)?;
        let feet = fields.number().ok_or_else(incomplete)?;
        record.frequency = fields.integer().ok_or_else(incomplete)?;
        record.service_class = fields.integer().ok_or_else(incomplete)?;
        record.parameter = fields.number().ok_or_else(incomplete)?;
        record.ident = fields.word().ok_or_else(incomplete)?.to_vec();
        record.airport = fields.word().ok_or_else(incomplete)?.to_vec();
        record.region = fields.word().ok_or_else(incomplete)?.to_vec();
        if !record.position.latitude.is_finite()
            || !record.position.longitude.is_finite()
            || !feet.is_finite()
            || !record.parameter.is_finite()
            || record.position.latitude.abs() > 90.
            || record.position.longitude.abs() > 180.
            || feet.abs() > 100000.
            || record.frequency < 0
            || record.service_class < 0
            || record.ident.is_empty()
            || record.ident.len() > 5
            || record.region.len() != 2
        {
            return Err(bad("invalid radio field"));
        }
        record.position.altitude = feet * 0.3048;
        if (4..=9).contains(&row) {
            record.runway = fields
                .word()
                .ok_or_else(|| bad("missing runway association"))?
                .to_vec();
        }
        record.name = fields.rest().to_vec();
        if record.name.is_empty() {
            return Err(bad("missing radio name"));
        }
        if row == 2 {
            if record.frequency == 0 || record.frequency > 10000 {
                return Err(bad("invalid NDB kHz frequency"));
            }
        } else if (7..=9).contains(&row) {
            if record.frequency != 0 || record.service_class != 0 {
                return Err(bad("marker frequency/range must be zero"));
            }
        } else if !(10800..=13699).contains(&record.frequency) {
            return Err(bad("invalid VHF paired frequency"));
        }
        if !keys.insert(record.key()) {
            let Some(skip) = duplicate.as_deref_mut() else {
                return Err(bad("duplicate identity within one navigation layer"));
            };
            let message = [
                source,
                b":",
                line_number.to_string().as_bytes(),
                b": duplicate identity ",
                &record.key(),
                b" skipped (the layer's first one is kept)",
            ]
            .concat();
            skip(&message);
            continue;
        }
        file.records.push(record);
    }
    if phase != 2 || !ended {
        return Err(Error {
            source: source.to_vec(),
            line: line_number,
            reason: "incomplete navigation file",
        });
    }
    Ok(file)
}

/// Later layers replace a matching identity in its first position. New
/// identities append in layer order; no geographic or frequency sorting occurs.
pub fn merge(files: &[File]) -> Vec<Record> {
    let mut records = vec![];
    let mut positions = BTreeMap::new();
    for file in files {
        for record in &file.records {
            let key = record.key();
            if let Some(&position) = positions.get(&key) {
                records[position] = record.clone();
            } else {
                positions.insert(key, records.len());
                records.push(record.clone());
            }
        }
    }
    records
}

#[cfg(all(test, windows, target_env = "msvc"))]
mod tests {
    use super::*;
    #[test]
    fn bom_text_bytes_runway_and_ignored_rows_are_preserved() {
        let data=b"\xef\xbb\xbfI\r\n1200 Version metadata\r\n4 -0 1 10.25 11030 25 0 IAAA KSEA K1 16L NAME\xff \t\r\n14 ignored\r\n99\r\n";
        let file = parse(data, b"layer", &MsvcNumbers, None).unwrap();
        assert_eq!(file.version, 1200);
        assert_eq!(file.ignored_rows, 1);
        assert_eq!(file.records.len(), 1);
        let record = &file.records[0];
        assert_eq!(record.position.latitude.to_bits(), (-0.0f64).to_bits());
        assert_eq!(record.runway, b"16L");
        assert_eq!(record.name, b"NAME\xff \t");
        assert_eq!(record.line, 3);
        assert_eq!(record.key(), b"4|IAAA|KSEA|K1|16L");
    }
    #[test]
    fn duplicate_callback_runs_before_requesting_another_line() {
        let lines = [
            b"I".as_slice(),
            b"1200 Version",
            b"4 0 0 0 11030 25 0 IAAA KSEA K1 16L FIRST",
            b"5 0 0 0 11030 25 0 IAAA KSEA K1 16L SECOND",
            b"99",
        ];
        let consumed = std::cell::Cell::new(0);
        let mut reported = vec![];
        let file = parse_lines(
            lines
                .into_iter()
                .inspect(|_| consumed.set(consumed.get() + 1)),
            b"layer",
            &MsvcNumbers,
            Some(&mut |message| {
                assert_eq!(consumed.get(), 4);
                reported.push(message.to_vec());
            }),
        )
        .unwrap();
        assert_eq!(file.records.len(), 1);
        assert_eq!(file.records[0].name, b"FIRST");
        assert_eq!(reported,[b"layer:4: duplicate identity 4|IAAA|KSEA|K1|16L skipped (the layer's first one is kept)".to_vec()]);
    }
    #[test]
    fn strict_duplicates_and_content_after_terminator_report_their_lines() {
        let data=b"I\n1200 Version\n3 0 0 0 11300 40 0 ID ENRT K1 FIRST\n3 1 2 0 11310 40 0 ID ENRT K1 SECOND\n99\n";
        assert_eq!(
            parse(data, b"base", &MsvcNumbers, None)
                .unwrap_err()
                .message(),
            b"base:4: duplicate identity within one navigation layer"
        );
        assert_eq!(
            parse(
                b"I\n1200 Version\n99\n \t\r\n14 ignored\n",
                b"base",
                &MsvcNumbers,
                None
            )
            .unwrap_err()
            .line,
            5
        );
    }
    #[test]
    fn later_layer_replaces_without_reordering_other_identities() {
        let a=parse(b"I\n1200 Version\n3 0 0 0 11300 40 0 A ENRT K1 FIRST\n2 0 0 0 362 25 0 B ENRT K1 SECOND\n99\n",b"base",&MsvcNumbers,None).unwrap();
        let b=parse(b"A\n1150 Version\n3 1 2 10 11310 40 0 A ENRT K1 OVERRIDE\n2 0 0 0 365 25 0 C ENRT K1 NEW\n99\n",b"user",&MsvcNumbers,None).unwrap();
        let rows = merge(&[a, b]);
        assert_eq!(
            rows.iter().map(|r| r.ident.as_slice()).collect::<Vec<_>>(),
            [b"A", b"B", b"C"]
        );
        assert_eq!(rows[0].source, b"user");
        assert_eq!(rows[0].frequency, 11310);
    }
}
