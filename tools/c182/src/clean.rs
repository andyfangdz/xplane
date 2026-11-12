//! Retention for the workspace's ignored `out/` scratch.
//!
//! Dry-run by default. Only these kinds of directory are candidates: leaked
//! oracle test temp folders, package sets beyond the newest few retained by
//! `package` (plus abandoned staging assemblies), marked headless case copies,
//! stray Cargo target directories (a top-level folder holding Cargo's
//! `CACHEDIR.TAG` and `.rustc_info.json`), and top-level trees that hold no
//! file. `--cargo` adds `out/cargo/debug`, the development build cache that
//! `test-fast` rebuilds. `out/packages`, the rest of `out/cargo`,
//! `out/acceptance` (`build.ps1 accept` records), `out/install-backups` (what
//! `install` replaced in the simulator) and any directory holding an
//! acceptance `manifest.json` are never removed. Loose files directly under
//! `out/` are left alone: older manifests name some of them. Reparse points
//! (junctions, symlinks) are never followed: a link is removed as a link, and
//! its target is untouched.
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// Top-level `out/` entries that are never candidates or walked.
const PROTECTED: [&str; 5] = [
    "packages",
    "cargo",
    "staging",
    "acceptance",
    "install-backups",
];
/// Package sets kept under `out/staging/previous-*`, newest first.
const KEEP_PREVIOUS: usize = 2;
/// A temp folder this new may belong to a test that is still running.
const TEMP_MIN_AGE: Duration = Duration::from_secs(2 * 60 * 60);
/// How deep below `out/` to look for headless case markers.
const CASE_SEARCH_DEPTH: usize = 4;
const CASE_MARKER: &str = ".headless-case.json";

/// Leaked oracle-test folders: prefix, then '-'-separated fields that are
/// d = decimal, x = lowercase hex, a = lowercase letters.
const TEMP_PATTERNS: [(&str, &str); 7] = [
    ("actual-visual-events-", "dxd"),
    ("actual-gauge-", "dx"),
    ("actual-radio-", "dx"),
    ("actual-avionics-", "dx"),
    ("gauge-files-proof-", "d"),
    ("receiver-files-", "ad"),
    ("xml-oracle-", "dd"),
];

/// Files Cargo writes at the root of every target directory.
const CARGO_TARGET_MARKERS: [&str; 2] = ["CACHEDIR.TAG", ".rustc_info.json"];

pub struct Options {
    pub apply: bool,
    pub days: u64,
    /// Also remove `out/cargo/debug`, the development build cache.
    pub cargo: bool,
}

struct Candidate {
    path: PathBuf,
    kind: &'static str,
    bytes: u64,
}

#[derive(Default)]
struct Measure {
    bytes: u64,
    acceptance: bool,
}

#[derive(Default)]
struct Plan {
    candidates: Vec<Candidate>,
    protected: Vec<PathBuf>,
    /// Top-level folders losing case copies; removed too when left empty.
    emptied: Vec<PathBuf>,
}

impl Plan {
    fn consider(&mut self, path: PathBuf, kind: &'static str) -> io::Result<()> {
        let measure = measure(&path)?;
        if measure.acceptance {
            self.protected.push(path);
        } else {
            self.candidates.push(Candidate {
                path,
                kind,
                bytes: measure.bytes,
            });
        }
        Ok(())
    }
}

pub fn clean(out: &Path, options: &Options) -> io::Result<()> {
    let meta = fs::symlink_metadata(out)?;
    if is_reparse(&meta) || !meta.is_dir() {
        return Err(io::Error::other("out/ must be a real directory"));
    }
    let now = SystemTime::now();
    let case_age = Duration::from_secs(options.days.saturating_mul(24 * 60 * 60));
    let mut plan = Plan::default();

    let mut tops: Vec<_> = fs::read_dir(out)?.collect::<io::Result<_>>()?;
    tops.sort_by_key(|e| e.file_name());
    for entry in &tops {
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = fs::symlink_metadata(entry.path())?;
        if PROTECTED.contains(&name.as_str()) || !meta.is_dir() {
            continue;
        }
        if is_temp(&name) {
            if age(&meta, now) >= TEMP_MIN_AGE {
                plan.consider(entry.path(), "leaked test temp folder")?;
            }
        } else if !is_reparse(&meta) {
            if is_cargo_target(&entry.path()) {
                if age(&meta, now) >= case_age {
                    plan.consider(entry.path(), "stray Cargo target directory")?;
                }
                continue;
            }
            let mut cases = Vec::new();
            if find_cases(&entry.path(), 1, &mut cases)? {
                plan.protected.push(entry.path());
                continue;
            }
            let mut emptied = false;
            for case in cases {
                let marker = fs::symlink_metadata(case.join(CASE_MARKER))?;
                let newest = age(&fs::symlink_metadata(&case)?, now).min(age(&marker, now));
                if newest >= case_age {
                    plan.consider(case, "headless case copy")?;
                    emptied = true;
                }
            }
            if emptied {
                // May hold nothing once its case copies are gone.
                plan.emptied.push(entry.path());
            } else if age(&meta, now) >= TEMP_MIN_AGE && !holds_file(&entry.path())? {
                plan.consider(entry.path(), "empty directory")?;
            }
        }
    }
    if options.cargo {
        let debug = out.join("cargo/debug");
        if fs::symlink_metadata(&debug).is_ok_and(|m| m.is_dir() && !is_reparse(&m)) {
            plan.consider(debug, "development build cache (test-fast rebuilds it)")?;
        }
    }

    let staging = out.join("staging");
    if fs::symlink_metadata(&staging).is_ok_and(|m| m.is_dir() && !is_reparse(&m)) {
        let mut previous = Vec::new();
        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = fs::symlink_metadata(entry.path())?;
            if !meta.is_dir() || is_reparse(&meta) {
                continue;
            }
            if let Some(stamp) = name
                .strip_prefix("previous-")
                .and_then(|s| s.parse::<u128>().ok())
            {
                previous.push((stamp, entry.path()));
            } else if name.parse::<u128>().is_ok() && age(&meta, now) >= case_age {
                plan.consider(entry.path(), "abandoned staging assembly")?;
            }
        }
        previous.sort_by_key(|(stamp, _)| std::cmp::Reverse(*stamp));
        for (_, path) in previous.into_iter().skip(KEEP_PREVIOUS) {
            plan.consider(path, "older previous package set")?;
        }
    }

    let Plan {
        candidates,
        protected,
        emptied,
    } = plan;
    let total: u64 = candidates.iter().map(|c| c.bytes).sum();
    let verb = if options.apply {
        "Removing"
    } else {
        "Would remove"
    };
    for candidate in &candidates {
        println!(
            "{verb} {:>9} {} ({})",
            gib(candidate.bytes),
            relative(out, &candidate.path),
            candidate.kind
        );
    }
    for path in &protected {
        println!("Kept (acceptance manifest) {}", relative(out, path));
    }
    let mut kinds: Vec<&str> = candidates.iter().map(|c| c.kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    for kind in kinds {
        let (count, bytes) = candidates
            .iter()
            .filter(|c| c.kind == kind)
            .fold((0, 0), |(n, b), c| (n + 1, b + c.bytes));
        println!("{count:>5} x {kind}: {}", gib(bytes));
    }
    let summary = format!(
        "{} directories, {} logical (hard links and ReFS block clones free less); case-copy and staging age limit {} days",
        candidates.len(),
        gib(total),
        options.days
    );
    if !options.apply {
        println!("Dry run: would remove {summary}.");
        if !emptied.is_empty() {
            println!(
                "{} folders losing case copies are removed too if that leaves them empty.",
                emptied.len()
            );
        }
        println!("Nothing was deleted. Re-run with --apply (build.ps1 clean -Apply) to delete.");
        return Ok(());
    }
    for candidate in &candidates {
        remove_tree(&candidate.path)?;
    }
    let mut left_empty = 0;
    for path in &emptied {
        if !holds_file(path)? {
            remove_tree(path)?;
            left_empty += 1;
        }
    }
    println!("Removed {summary}.");
    if left_empty > 0 {
        println!("Removed {left_empty} folders left empty.");
    }
    Ok(())
}

fn is_cargo_target(dir: &Path) -> bool {
    CARGO_TARGET_MARKERS
        .iter()
        .all(|name| fs::symlink_metadata(dir.join(name)).is_ok_and(|m| m.is_file()))
}

/// True when the tree holds any file or link. Links are never followed.
fn holds_file(dir: &Path) -> io::Result<bool> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_dir() || is_reparse(&meta) || holds_file(&path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_temp(name: &str) -> bool {
    TEMP_PATTERNS.iter().any(|(prefix, fields)| {
        name.strip_prefix(prefix).is_some_and(|rest| {
            let parts: Vec<&str> = rest.split('-').collect();
            parts.len() == fields.len()
                && parts.iter().zip(fields.chars()).all(|(part, class)| {
                    !part.is_empty()
                        && part.chars().all(|c| match class {
                            'd' => c.is_ascii_digit(),
                            'x' => c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
                            _ => c.is_ascii_lowercase(),
                        })
                })
        })
    })
}

/// Collects marked case copies below `dir` without entering them. Returns true
/// when an acceptance manifest is found, which protects the whole tree.
fn find_cases(dir: &Path, depth: usize, cases: &mut Vec<PathBuf>) -> io::Result<bool> {
    if is_acceptance(&dir.join("manifest.json")) {
        return Ok(true);
    }
    if depth > CASE_SEARCH_DEPTH {
        return Ok(false);
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_dir() || is_reparse(&meta) {
            continue;
        }
        if fs::symlink_metadata(path.join(CASE_MARKER)).is_ok_and(|m| m.is_file()) {
            cases.push(path);
        } else if find_cases(&path, depth + 1, cases)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn measure(path: &Path) -> io::Result<Measure> {
    let mut total = Measure::default();
    let meta = fs::symlink_metadata(path)?;
    if is_reparse(&meta) {
        return Ok(total);
    }
    if !meta.is_dir() {
        total.bytes = meta.len();
        total.acceptance = path
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("manifest.json"))
            && is_acceptance(path);
        return Ok(total);
    }
    for entry in fs::read_dir(path)? {
        let inner = measure(&entry?.path())?;
        total.bytes += inner.bytes;
        total.acceptance |= inner.acceptance;
    }
    Ok(total)
}

/// An acceptance record (`tools/record_rust_acceptance.py`, any schema) carries a
/// top-level `complete` field; package and case manifests do not.
fn is_acceptance(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !is_reparse(&m))
        && fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|value| value.get("complete").is_some())
}

/// Deletes a tree without following reparse points: a junction or symlink is
/// removed itself, never its target's contents.
fn remove_tree(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if is_reparse(&meta) {
        return if reparse_is_directory(&meta) {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        };
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            remove_tree(&entry?.path())?;
        }
        return fs::remove_dir(path);
    }
    if meta.permissions().readonly() {
        let mut permissions = meta.permissions();
        // Clears the Windows read-only attribute on a scratch file we own.
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    fs::remove_file(path)
}

fn age(meta: &fs::Metadata, now: SystemTime) -> Duration {
    meta.modified()
        .ok()
        .and_then(|t| now.duration_since(t).ok())
        .unwrap_or_default()
}

fn is_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn reparse_is_directory(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x10 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

fn gib(bytes: u64) -> String {
    format!("{:.2} GiB", bytes as f64 / (1u64 << 30) as f64)
}

fn relative(out: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(out).unwrap_or(path);
    format!("out/{}", rel.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_patterns_match_only_leaked_test_folders() {
        for name in [
            "actual-visual-events-81352-1f80-0",
            "actual-gauge-7388-1f80",
            "actual-radio-81984-9f80",
            "actual-avionics-89360-1f80",
            "gauge-files-proof-12140",
            "receiver-files-models-60260",
            "xml-oracle-5664-1794346042420999200",
        ] {
            assert!(is_temp(name), "{name}");
        }
        for name in [
            "actual-gauge",
            "actual-gauge-7388",
            "actual-gauge-7388-1F80",
            "actual-gauge-7388-1f80-extra",
            "receiver-files-models-",
            "xml-oracle-proof",
            "gauge-files-proof-final",
            "public-aircraft-harness-acceptance",
            "packages",
        ] {
            assert!(!is_temp(name), "{name}");
        }
    }

    #[test]
    fn dry_run_keeps_protected_trees_and_reports_candidates() {
        let root = std::env::temp_dir().join(format!("c182-clean-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let out = root.join("out");
        for dir in [
            "packages/C182_Converted",
            "cargo/release",
            "staging/previous-1",
            "staging/previous-2",
            "staging/previous-3",
            "actual-gauge-1-1f80",
            "campaign/headless-cases/steam",
            "old-cases/steam",
            "acceptance/attempt/headless-cases/steam",
            "install-backups/20261111T000000Z/files",
        ] {
            fs::create_dir_all(out.join(dir)).unwrap();
        }
        fs::write(
            out.join("campaign/manifest.json"),
            br#"{"schema":3,"complete":true}"#,
        )
        .unwrap();
        fs::write(
            out.join("campaign/headless-cases/steam").join(CASE_MARKER),
            b"{}",
        )
        .unwrap();
        fs::write(out.join("old-cases/steam").join(CASE_MARKER), b"{}").unwrap();
        fs::write(
            out.join("acceptance/attempt/headless-cases/steam")
                .join(CASE_MARKER),
            b"{}",
        )
        .unwrap();
        fs::write(
            out.join("old-cases/steam/manifest.json"),
            br#"{"schema":"package"}"#,
        )
        .unwrap();
        for dir in [
            "cargo/debug/deps",
            "stray-target/debug",
            "fresh-empty/inner",
        ] {
            fs::create_dir_all(out.join(dir)).unwrap();
        }
        fs::write(out.join("cargo/debug/deps/unit.rlib"), b"x").unwrap();
        for marker in CARGO_TARGET_MARKERS {
            fs::write(out.join("stray-target").join(marker), b"x").unwrap();
        }
        assert!(is_cargo_target(&out.join("stray-target")));
        assert!(!is_cargo_target(&out.join("old-cases")));
        assert!(holds_file(&out.join("old-cases")).unwrap());
        assert!(!holds_file(&out.join("fresh-empty")).unwrap());
        // Freshly created temp and empty folders are inside the running-test guard.
        clean(
            &out,
            &Options {
                apply: true,
                days: 0,
                cargo: false,
            },
        )
        .unwrap();
        assert!(out.join("actual-gauge-1-1f80").is_dir());
        assert!(out.join("fresh-empty/inner").is_dir());
        assert!(out.join("packages/C182_Converted").is_dir());
        assert!(out.join("cargo/release").is_dir());
        assert!(out.join("cargo/debug/deps/unit.rlib").is_file());
        assert!(!out.join("stray-target").exists());
        assert!(out.join("campaign/headless-cases/steam").is_dir());
        // The case copy goes, and its folder with it once that leaves it empty.
        assert!(!out.join("old-cases").exists());
        clean(
            &out,
            &Options {
                apply: true,
                days: 0,
                cargo: true,
            },
        )
        .unwrap();
        assert!(!out.join("cargo/debug").exists());
        assert!(out.join("cargo/release").is_dir());
        assert!(out.join("acceptance/attempt/headless-cases/steam").is_dir());
        assert!(out.join("install-backups/20261111T000000Z/files").is_dir());
        assert!(out.join("staging/previous-3").is_dir());
        assert!(out.join("staging/previous-2").is_dir());
        assert!(!out.join("staging/previous-1").exists());
        fs::remove_dir_all(&root).unwrap();
    }
}
