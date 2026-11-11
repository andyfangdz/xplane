//! Install the verified package set into the simulator's aircraft folder.
//!
//! `install` refuses while an X-Plane process runs or unless `out/packages` is
//! the verified package set of the current build. It backs up every file it
//! will replace to `out/install-backups/<utc>/` before copying, verifies every
//! installed file against the package manifest, and writes `installed.json`
//! into each aircraft folder and `out/installed.json`. `rollback` restores the
//! newest backup of a root that has not been rolled back, and `installed`
//! reports drift between the simulator folders, their records and
//! `out/packages`. Simulator `source-state` folders are live aircraft state:
//! the manifest excludes them and nothing here writes them.
use crate::{Manifest, Result, Workspace, error, hash, is_link};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

/// Provenance record written into each installed aircraft folder.
pub const RECORD: &str = "installed.json";
const BACKUP: &str = "backup.json";
const ROLLED_BACK: &str = "rolled-back.json";
/// Never compared, copied or counted as extra: live simulator state.
const STATE: &str = "source-state";

#[derive(Serialize, Deserialize)]
struct Backup {
    schema: u32,
    utc: String,
    root: String,
    /// Destination (relative to the root) -> SHA-256 of the replaced file,
    /// whose copy is kept under `files/`.
    replaced: BTreeMap<String, String>,
    /// Destinations the install created; rollback deletes them.
    created: Vec<String>,
    /// This root's entry in `out/installed.json` before the install.
    previous_record: Option<Value>,
}

#[derive(Debug, PartialEq)]
enum Action {
    Unchanged,
    Replace(String),
    Create,
}

struct Entry {
    relative: String,
    source: PathBuf,
    dest: PathBuf,
    sha256: String,
    action: Action,
}

#[derive(Default, Debug, PartialEq)]
struct Restored {
    restored: usize,
    removed: usize,
}

/// UTC now as an RFC 3339 time and a compact directory stamp.
fn utc_now() -> Result<(String, String)> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, m, s) = (rest / 3600, rest % 3600 / 60, rest % 60);
    Ok((
        format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z"),
        format!("{year:04}{month:02}{day:02}T{h:02}{m:02}{s:02}Z"),
    ))
}

/// X-Plane image names in `tasklist /FO CSV /NH` output.
fn simulator_processes(csv: &str) -> Vec<String> {
    csv.lines()
        .filter_map(|line| {
            let mut fields = line.split("\",\"");
            let image = fields.next()?.trim().trim_start_matches('"').to_owned();
            let pid = fields.next()?.trim_end_matches('"').to_owned();
            image
                .to_ascii_lowercase()
                .starts_with("x-plane")
                .then(|| format!("{image} (PID {pid})"))
        })
        .collect()
}

fn running_simulators() -> Result<Vec<String>> {
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| error(format!("Cannot list processes ({e}); refusing")))?;
    if !output.status.success() {
        return Err(error("Cannot list processes; refusing"));
    }
    Ok(simulator_processes(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn refuse_while_simulator_runs() -> Result<()> {
    let running = running_simulators()?;
    if running.is_empty() {
        Ok(())
    } else {
        Err(error(format!(
            "X-Plane is running ({}); quit it first",
            running.join(", ")
        )))
    }
}

fn short(sha: &str) -> &str {
    sha.get(..8).unwrap_or(sha)
}

/// Refuse a destination below `root` that passes through a link.
fn check_no_links(root: &Path, dest: &Path, checked: &mut BTreeSet<PathBuf>) -> Result<()> {
    let relative = dest
        .strip_prefix(root)
        .map_err(|_| error(format!("{} is outside {}", dest.display(), root.display())))?;
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        if checked.contains(&path) {
            continue;
        }
        if fs::symlink_metadata(&path).is_ok() && is_link(&path)? {
            return Err(error(format!(
                "Refusing to install through a link: {}",
                path.display()
            )));
        }
        checked.insert(path.clone());
    }
    Ok(())
}

fn plan(packages: &Path, files: &BTreeMap<String, String>, root: &Path) -> Result<Vec<Entry>> {
    let mut checked = BTreeSet::new();
    let mut entries = Vec::with_capacity(files.len());
    for (relative, sha256) in files {
        if relative
            .split('/')
            .any(|part| part == STATE || part == "..")
        {
            return Err(error(format!("Unexpected package path {relative}")));
        }
        let dest = root.join(relative);
        check_no_links(root, &dest, &mut checked)?;
        let action = if dest.is_file() {
            let current = hash(&dest)?;
            if &current == sha256 {
                Action::Unchanged
            } else {
                Action::Replace(current)
            }
        } else if dest.exists() {
            return Err(error(format!("{} is not a file", dest.display())));
        } else {
            Action::Create
        };
        entries.push(Entry {
            relative: relative.clone(),
            source: packages.join(relative),
            dest,
            sha256: sha256.clone(),
            action,
        });
    }
    Ok(entries)
}

fn copy_file(source: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Ok(meta) = fs::metadata(dest)
        && meta.permissions().readonly()
    {
        let mut permissions = meta.permissions();
        // Replacing an installed file the backup already holds.
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(dest, permissions)?;
    }
    fs::copy(source, dest)?;
    Ok(())
}

fn copy_verified(source: &Path, dest: &Path, sha256: &str) -> Result<()> {
    copy_file(source, dest)?;
    if hash(dest)? != sha256 {
        return Err(error(format!("Copy does not verify: {}", dest.display())));
    }
    Ok(())
}

/// Back up what `entries` and the folder records will replace, then write
/// `backup.json` before any destination is touched.
fn back_up(
    root: &Path,
    entries: &[Entry],
    records: &[String],
    backup: &Path,
    previous_record: Option<Value>,
    utc: &str,
) -> Result<Backup> {
    let files = backup.join("files");
    let mut record = Backup {
        schema: 1,
        utc: utc.to_owned(),
        root: root.to_string_lossy().into_owned(),
        replaced: BTreeMap::new(),
        created: Vec::new(),
        previous_record,
    };
    for entry in entries {
        match &entry.action {
            Action::Unchanged => {}
            Action::Create => record.created.push(entry.relative.clone()),
            Action::Replace(old) => {
                copy_verified(&entry.dest, &files.join(&entry.relative), old)?;
                record.replaced.insert(entry.relative.clone(), old.clone());
            }
        }
    }
    for relative in records {
        let dest = root.join(relative);
        if dest.is_file() {
            let old = hash(&dest)?;
            copy_verified(&dest, &files.join(relative), &old)?;
            record.replaced.insert(relative.clone(), old);
        } else {
            record.created.push(relative.clone());
        }
    }
    fs::write(backup.join(BACKUP), serde_json::to_vec_pretty(&record)?)?;
    Ok(record)
}

fn remove_empty_parents(root: &Path, path: &Path) {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == root || !dir.starts_with(root) || fs::remove_dir(dir).is_err() {
            break;
        }
        current = dir.parent();
    }
}

/// Restore one backup over its root: replaced files return, created files go.
fn restore(backup: &Path) -> Result<Restored> {
    let record: Backup = serde_json::from_slice(&fs::read(backup.join(BACKUP))?)?;
    let root = PathBuf::from(&record.root);
    let mut checked = BTreeSet::new();
    let mut result = Restored::default();
    for (relative, sha256) in &record.replaced {
        let dest = root.join(relative);
        check_no_links(&root, &dest, &mut checked)?;
        copy_verified(&backup.join("files").join(relative), &dest, sha256)?;
        result.restored += 1;
    }
    for relative in &record.created {
        let dest = root.join(relative);
        check_no_links(&root, &dest, &mut checked)?;
        if dest.is_file() {
            fs::remove_file(&dest)?;
            remove_empty_parents(&root.join(relative.split('/').next().unwrap_or("")), &dest);
            result.removed += 1;
        }
    }
    Ok(result)
}

/// Copy and verify the planned files; on any failure restore the backup.
fn apply(entries: &[Entry], backup: &Path) -> Result<()> {
    let attempt = || -> Result<()> {
        for entry in entries.iter().filter(|e| e.action != Action::Unchanged) {
            copy_file(&entry.source, &entry.dest)?;
        }
        for entry in entries {
            if hash(&entry.dest)? != entry.sha256 {
                return Err(error(format!(
                    "Installed file does not verify: {}",
                    entry.dest.display()
                )));
            }
        }
        Ok(())
    };
    if let Err(failure) = attempt() {
        let (utc, _) = utc_now()?;
        return match restore(backup) {
            Ok(done) => {
                fs::write(
                    backup.join(ROLLED_BACK),
                    serde_json::to_vec_pretty(&json!({
                        "utc": utc, "reason": format!("install failed: {failure}"),
                        "restored": done.restored, "removed": done.removed,
                    }))?,
                )?;
                Err(error(format!(
                    "Install failed and was rolled back from {}: {failure}",
                    backup.display()
                )))
            }
            Err(restore_error) => Err(error(format!(
                "Install failed ({failure}) and restoring {} also failed ({restore_error}); run rollback",
                backup.display()
            ))),
        };
    }
    Ok(())
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// `out/installed.json`: the latest install record per simulator root.
fn read_installs(out: &Path) -> BTreeMap<String, Value> {
    read_json(&out.join(RECORD))
        .and_then(|v| v.get("installs").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn write_installs(out: &Path, installs: &BTreeMap<String, Value>) -> Result<()> {
    let path = out.join(RECORD);
    let staged = out.join("installed.json.tmp");
    fs::write(
        &staged,
        serde_json::to_vec_pretty(&json!({"schema": 1, "installs": installs}))?,
    )?;
    fs::rename(staged, path)?;
    Ok(())
}

fn folders(files: &BTreeMap<String, String>) -> BTreeSet<String> {
    files
        .keys()
        .filter_map(|k| k.split('/').next().map(str::to_owned))
        .collect()
}

/// Newest backup of `root` that has not been rolled back.
fn latest_backup(backups: &Path, root: &Path) -> Result<PathBuf> {
    let mut found = Vec::new();
    if backups.is_dir() {
        for entry in fs::read_dir(backups)? {
            let path = entry?.path();
            if !path.join(ROLLED_BACK).exists()
                && let Some(record) = read_json(&path.join(BACKUP))
                && record.get("root").and_then(Value::as_str) == Some(&*root.to_string_lossy())
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found.pop().ok_or_else(|| {
        error(format!(
            "No backup of {} remains to roll back under {}",
            root.display(),
            backups.display()
        ))
    })
}

impl Workspace {
    fn public_root(&self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The simulator aircraft root: `--root` when given, else `[install] root`.
    pub fn install_root(&self, root: Option<&str>) -> Result<PathBuf> {
        let path = match root {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(
                self.config
                    .install
                    .get("root")
                    .ok_or_else(|| error("Missing [install] root in c182.local.toml"))?,
            ),
        };
        let path = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        Ok(std::path::absolute(path)?)
    }

    fn acceptance(&self, plugin: &str) -> Value {
        let path = self.root.join("out/acceptance/latest.json");
        match read_json(&path) {
            Some(latest) => json!({
                "latest": path.to_string_lossy(),
                "plugin_sha256": latest.get("plugin_sha256"),
                "plugin_matches": latest.get("plugin_sha256").and_then(Value::as_str) == Some(plugin),
            }),
            None => Value::Null,
        }
    }

    /// The verified package set of the current build, or an explanation.
    fn current_package(&self) -> Result<(PathBuf, Manifest, String, String)> {
        let packages = self.output("packages")?;
        let manifest_path = packages.join("private-manifest.json");
        let manifest: Manifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        self.verify(&packages)?;
        let built = self.root.join("out/cargo/release/c182_plugin.dll");
        let plugin = hash(&built)
            .map_err(|e| error(format!("No current build at {} ({e})", built.display())))?;
        for folder in folders(&manifest.files) {
            let packaged = manifest
                .files
                .get(&format!("{folder}/{}", self.package.plugin.binary))
                .ok_or_else(|| error(format!("{folder} has no plugin")))?;
            if packaged != &plugin {
                return Err(error(format!(
                    "{folder} holds plugin {}, the current build is {}; run build.ps1 package",
                    short(packaged),
                    short(&plugin)
                )));
            }
        }
        let head = self.git(&["rev-parse", "HEAD"]);
        if manifest.source_revision != head || !manifest.source_status.is_empty() {
            return Err(error(format!(
                "out/packages was built from private {} ({}), not the clean HEAD {}; package again",
                short(&manifest.source_revision),
                if manifest.source_status.is_empty() {
                    "clean"
                } else {
                    "dirty"
                },
                short(&head)
            )));
        }
        let public = self.git_at(&self.public_root(), &["rev-parse", "HEAD"]);
        if manifest.public_revision != public {
            return Err(error(format!(
                "out/packages was built from public {}, the public HEAD is {}; package again",
                short(&manifest.public_revision),
                short(&public)
            )));
        }
        Ok((packages, manifest, hash(&manifest_path)?, plugin))
    }

    pub fn install(&self, root: Option<&str>) -> Result<()> {
        refuse_while_simulator_runs()?;
        let root = self.install_root(root)?;
        if !root.is_dir() {
            return Err(error(format!(
                "Install root {} does not exist",
                root.display()
            )));
        }
        let (packages, manifest, manifest_sha256, plugin) = self.current_package()?;
        let out = self.root.join("out");
        let entries = plan(&packages, &manifest.files, &root)?;
        let folders = folders(&manifest.files);
        let records: Vec<String> = folders.iter().map(|f| format!("{f}/{RECORD}")).collect();
        let (utc, stamp) = utc_now()?;
        let backup = out.join("install-backups").join(&stamp);
        fs::create_dir_all(backup.parent().unwrap())?;
        fs::create_dir(&backup)
            .map_err(|e| error(format!("Backup {} exists ({e})", backup.display())))?;
        let mut installs = read_installs(&out);
        let key = root.to_string_lossy().into_owned();
        // Re-check just before the first write to the simulator tree.
        refuse_while_simulator_runs()?;
        let saved = back_up(
            &root,
            &entries,
            &records,
            &backup,
            installs.get(&key).cloned(),
            &utc,
        )?;
        println!(
            "Backed up {} files to {}",
            saved.replaced.len(),
            backup.display()
        );
        apply(&entries, &backup)?;
        let count =
            |wanted: fn(&Action) -> bool| entries.iter().filter(|e| wanted(&e.action)).count();
        let record = json!({
            "schema": 1,
            "installed_utc": utc,
            "root": key,
            "folders": folders,
            "plugin_sha256": plugin,
            "package_manifest_sha256": manifest_sha256,
            "implementation": manifest.implementation,
            "private_head": self.git(&["rev-parse", "HEAD"]),
            "public_head": self.git_at(&self.public_root(), &["rev-parse", "HEAD"]),
            "package_source_revision": manifest.source_revision,
            "package_public_revision": manifest.public_revision,
            "package_public_status": manifest.public_status,
            "files": {
                "verified": entries.len(),
                "replaced": count(|a| matches!(a, Action::Replace(_))),
                "created": count(|a| *a == Action::Create),
                "unchanged": count(|a| *a == Action::Unchanged),
            },
            "backup": backup.to_string_lossy(),
            "acceptance": self.acceptance(&plugin),
            "state": "Simulator source-state folders are live aircraft state and were not changed.",
        });
        for relative in &records {
            fs::write(root.join(relative), serde_json::to_vec_pretty(&record)?)?;
        }
        installs.insert(key, record.clone());
        write_installs(&out, &installs)?;
        println!(
            "Installed plugin {} and {} verified package files ({} replaced, {} created) into {} folders under {}",
            short(&plugin),
            entries.len(),
            record["files"]["replaced"],
            record["files"]["created"],
            folders.len(),
            root.display()
        );
        Ok(())
    }

    pub fn rollback(&self, root: Option<&str>) -> Result<()> {
        refuse_while_simulator_runs()?;
        let root = self.install_root(root)?;
        let out = self.root.join("out");
        let backup = latest_backup(&out.join("install-backups"), &root)?;
        let record: Backup = serde_json::from_slice(&fs::read(backup.join(BACKUP))?)?;
        let done = restore(&backup)?;
        let (utc, _) = utc_now()?;
        fs::write(
            backup.join(ROLLED_BACK),
            serde_json::to_vec_pretty(&json!({
                "utc": utc, "reason": "rollback", "restored": done.restored, "removed": done.removed,
            }))?,
        )?;
        let mut installs = read_installs(&out);
        let key = root.to_string_lossy().into_owned();
        let note = json!({"utc": utc, "backup": backup.to_string_lossy(), "restored": done.restored, "removed": done.removed});
        match record.previous_record {
            Some(Value::Object(mut previous)) => {
                previous.insert("restored_by_rollback".into(), note);
                installs.insert(key, Value::Object(previous));
            }
            _ => {
                installs.insert(
                    key,
                    json!({"schema": 1, "state": "no recorded install", "restored_by_rollback": note}),
                );
            }
        }
        write_installs(&out, &installs)?;
        println!(
            "Rolled back {}: restored {} files, removed {} files the install created, under {}",
            backup.display(),
            done.restored,
            done.removed,
            root.display()
        );
        Ok(())
    }

    /// Report drift between the simulator folders, their records and out/packages.
    pub fn installed(&self, root: Option<&str>) -> Result<()> {
        let root = self.install_root(root)?;
        let out = self.root.join("out");
        let mut drift = Vec::new();
        println!("Install root: {}", root.display());
        match running_simulators() {
            Ok(list) if list.is_empty() => println!("X-Plane processes: none"),
            Ok(list) => println!("X-Plane processes: {}", list.join(", ")),
            Err(e) => println!("X-Plane processes: unknown ({e})"),
        }
        let packages = self.root.join("out/packages");
        let manifest_path = packages.join("private-manifest.json");
        let manifest: Option<Manifest> = fs::read(&manifest_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let manifest_sha256 = hash(&manifest_path).ok();
        let package_plugin = manifest.as_ref().and_then(|m| {
            folders(&m.files).into_iter().next().and_then(|f| {
                m.files
                    .get(&format!("{f}/{}", self.package.plugin.binary))
                    .cloned()
            })
        });
        match (&manifest, &manifest_sha256) {
            (Some(m), Some(sha)) => println!(
                "out/packages: manifest {}, plugin {}, private {}{}, public {}",
                short(sha),
                package_plugin.as_deref().map_or("?", short),
                short(&m.source_revision),
                if m.source_status.is_empty() {
                    ""
                } else {
                    " (dirty)"
                },
                short(&m.public_revision)
            ),
            _ => println!("out/packages: none"),
        }
        let built = hash(&self.root.join("out/cargo/release/c182_plugin.dll")).ok();
        println!("Current build: {}", built.as_deref().map_or("none", short));
        if let Some(latest) = read_json(&out.join("acceptance/latest.json")) {
            println!(
                "Acceptance: out/acceptance/latest.json plugin {}",
                latest
                    .get("plugin_sha256")
                    .and_then(Value::as_str)
                    .map_or("?", short)
            );
        } else {
            println!("Acceptance: no out/acceptance/latest.json");
        }
        let installs = read_installs(&out);
        let last = installs.get(&*root.to_string_lossy());
        match last {
            Some(record) => println!(
                "out/installed.json: plugin {}, manifest {}, {}",
                record
                    .get("plugin_sha256")
                    .and_then(Value::as_str)
                    .map_or("none", short),
                record
                    .get("package_manifest_sha256")
                    .and_then(Value::as_str)
                    .map_or("none", short),
                record
                    .get("installed_utc")
                    .and_then(Value::as_str)
                    .or_else(|| record.get("state").and_then(Value::as_str))
                    .unwrap_or("?")
            ),
            None => println!("out/installed.json: no install recorded for this root"),
        }
        let expected: Vec<String> = match &manifest {
            Some(m) => folders(&m.files).into_iter().collect(),
            None => self
                .package
                .variants
                .iter()
                .map(|v| v.folder.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        };
        for folder in &expected {
            let dir = root.join(folder);
            println!("{folder}:");
            if !dir.is_dir() {
                println!("  not installed");
                drift.push(format!("{folder} is not installed"));
                continue;
            }
            let dll = hash(&dir.join(&self.package.plugin.binary)).ok();
            println!("  plugin: {}", dll.as_deref().map_or("missing", short));
            let record = read_json(&dir.join(RECORD));
            let recorded = record
                .as_ref()
                .and_then(|r| r.get("plugin_sha256"))
                .and_then(Value::as_str);
            match &record {
                Some(r) => println!(
                    "  installed.json: plugin {}, manifest {}, {}",
                    recorded.map_or("?", short),
                    r.get("package_manifest_sha256")
                        .and_then(Value::as_str)
                        .map_or("?", short),
                    r.get("installed_utc")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                ),
                None => println!("  installed.json: none"),
            }
            if let Some(assembly) = read_json(&dir.join("package-assembly.json")).and_then(|a| {
                a.get("plugin_sha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }) {
                println!("  package-assembly.json plugin: {}", short(&assembly));
                if dll.as_deref() != Some(&*assembly) {
                    drift.push(format!(
                        "{folder}: plugin {} but package-assembly.json names {}",
                        dll.as_deref().map_or("missing", short),
                        short(&assembly)
                    ));
                }
            }
            match recorded {
                None => drift.push(format!("{folder}: no installed.json provenance record")),
                Some(r) if dll.as_deref() != Some(r) => drift.push(format!(
                    "{folder}: plugin {} but installed.json names {}",
                    dll.as_deref().map_or("missing", short),
                    short(r)
                )),
                _ => {}
            }
            if let (Some(r), Some(last)) = (&record, last)
                && (r.get("plugin_sha256") != last.get("plugin_sha256")
                    || r.get("installed_utc") != last.get("installed_utc"))
            {
                drift.push(format!(
                    "{folder}: installed.json differs from out/installed.json"
                ));
            }
            let Some(m) = &manifest else { continue };
            if let (Some(r), Some(sha)) = (&record, &manifest_sha256)
                && r.get("package_manifest_sha256").and_then(Value::as_str) != Some(sha)
            {
                drift.push(format!(
                    "{folder}: installed package is not the current out/packages set"
                ));
            }
            let prefix = format!("{folder}/");
            let (mut same, mut differ, mut missing) = (0, Vec::new(), Vec::new());
            for (relative, sha256) in m.files.iter().filter(|(k, _)| k.starts_with(&prefix)) {
                match hash(&root.join(relative)) {
                    Ok(actual) if &actual == sha256 => same += 1,
                    Ok(_) => differ.push(relative.clone()),
                    Err(_) => missing.push(relative.clone()),
                }
            }
            let mut extra = Vec::new();
            extra_files(&root, Path::new(folder), &m.files, &mut extra)?;
            println!(
                "  files vs out/packages: {same} same, {} differ, {} missing; {} extra (outside the package; source-state and installed.json not counted)",
                differ.len(),
                missing.len(),
                extra.len()
            );
            for (label, list) in [("differs", &differ), ("missing", &missing)] {
                for relative in list.iter().take(8) {
                    println!("    {label}: {relative}");
                }
                if list.len() > 8 {
                    println!("    ... {} more {label}", list.len() - 8);
                }
            }
            if !differ.is_empty() || !missing.is_empty() {
                drift.push(format!(
                    "{folder}: {} files differ from and {} are missing against out/packages",
                    differ.len(),
                    missing.len()
                ));
            }
        }
        if let (Some(p), Some(b)) = (&package_plugin, &built)
            && p != b
        {
            drift.push(format!(
                "out/packages holds plugin {}, the current build is {}",
                short(p),
                short(b)
            ));
        }
        if drift.is_empty() {
            println!("No drift: the simulator folders, their records and out/packages agree.");
        } else {
            println!("Drift ({}):", drift.len());
            for line in &drift {
                println!("  {line}");
            }
        }
        Ok(())
    }
}

/// Files under `folder` that the package does not list, without following links.
fn extra_files(
    root: &Path,
    relative: &Path,
    files: &BTreeMap<String, String>,
    extra: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let rel = relative.join(entry.file_name());
        let name = rel.to_string_lossy().replace('\\', "/");
        if entry.file_name() == STATE
            || (rel.components().count() == 2 && entry.file_name() == RECORD)
        {
            continue;
        }
        if is_link(&entry.path())? {
            extra.push(name);
        } else if entry.file_type()?.is_dir() {
            extra_files(root, &rel, files, extra)?;
        } else if !files.contains_key(&name) {
            extra.push(name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn finds_only_simulator_processes() {
        let csv = "\"System\",\"4\",\"Services\",\"0\",\"1,234 K\"\r\n\
                   \"X-Plane.exe\",\"8812\",\"Console\",\"1\",\"9,000,000 K\"\r\n\
                   \"xplane-tools.exe\",\"9\",\"Console\",\"1\",\"1 K\"\r\n";
        assert_eq!(simulator_processes(csv), ["X-Plane.exe (PID 8812)"]);
        assert!(simulator_processes("").is_empty());
    }

    #[test]
    fn utc_stamp_is_well_formed() {
        let (iso, stamp) = utc_now().unwrap();
        assert_eq!(iso.len(), 20);
        assert!(iso.ends_with('Z') && iso.as_bytes()[10] == b'T');
        assert_eq!(stamp, iso.replace(['-', ':'], ""));
    }

    #[test]
    fn install_backs_up_verifies_and_rolls_back() {
        let base = std::env::temp_dir().join(format!("c182-install-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (packages, root, backup) = (
            base.join("packages"),
            base.join("root"),
            base.join("backup"),
        );
        let mut files = BTreeMap::new();
        for (relative, body) in [
            ("A/plugins/win.xpl", b"new plugin".as_slice()),
            ("A/same.txt", b"same"),
            ("A/new/created.txt", b"created"),
        ] {
            fs::create_dir_all(packages.join(relative).parent().unwrap()).unwrap();
            fs::write(packages.join(relative), body).unwrap();
            files.insert(relative.to_owned(), sha(body));
        }
        for (relative, body) in [
            ("A/plugins/win.xpl", b"old plugin".as_slice()),
            ("A/same.txt", b"same"),
            ("A/vendor.txt", b"vendor"),
            ("A/source-state/state.bin", b"live"),
            ("A/installed.json", b"{}"),
        ] {
            fs::create_dir_all(root.join(relative).parent().unwrap()).unwrap();
            fs::write(root.join(relative), body).unwrap();
        }
        let entries = plan(&packages, &files, &root).unwrap();
        let actions: Vec<_> = entries.iter().map(|e| &e.action).collect();
        assert_eq!(
            actions,
            [
                &Action::Create,
                &Action::Replace(sha(b"old plugin")),
                &Action::Unchanged
            ]
        );
        fs::create_dir_all(&backup).unwrap();
        let record = back_up(
            &root,
            &entries,
            &["A/installed.json".to_owned()],
            &backup,
            None,
            "2026-11-11T00:00:00Z",
        )
        .unwrap();
        assert_eq!(record.replaced.len(), 2);
        assert_eq!(record.created, ["A/new/created.txt"]);
        apply(&entries, &backup).unwrap();
        assert_eq!(
            fs::read(root.join("A/plugins/win.xpl")).unwrap(),
            b"new plugin"
        );
        fs::write(root.join("A/installed.json"), b"{\"new\":1}").unwrap();
        let mut extra = Vec::new();
        extra_files(&root, Path::new("A"), &files, &mut extra).unwrap();
        assert_eq!(extra, ["A/vendor.txt"]);
        assert_eq!(
            restore(&backup).unwrap(),
            Restored {
                restored: 2,
                removed: 1
            }
        );
        assert_eq!(
            fs::read(root.join("A/plugins/win.xpl")).unwrap(),
            b"old plugin"
        );
        assert_eq!(fs::read(root.join("A/installed.json")).unwrap(), b"{}");
        assert!(!root.join("A/new").exists());
        assert_eq!(
            fs::read(root.join("A/source-state/state.bin")).unwrap(),
            b"live"
        );
        assert_eq!(fs::read(root.join("A/vendor.txt")).unwrap(), b"vendor");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn failed_copy_restores_the_backup() {
        let base = std::env::temp_dir().join(format!("c182-install-fail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (packages, root, backup) = (
            base.join("packages"),
            base.join("root"),
            base.join("backup"),
        );
        let mut files = BTreeMap::new();
        for (relative, body) in [("A/a.txt", b"new a".as_slice()), ("A/b.txt", b"new b")] {
            fs::create_dir_all(packages.join(relative).parent().unwrap()).unwrap();
            fs::write(packages.join(relative), body).unwrap();
            files.insert(relative.to_owned(), sha(body));
        }
        fs::create_dir_all(root.join("A")).unwrap();
        fs::write(root.join("A/a.txt"), b"old a").unwrap();
        let entries = plan(&packages, &files, &root).unwrap();
        // The second source disappears after planning, so its copy fails.
        fs::remove_file(packages.join("A/b.txt")).unwrap();
        fs::create_dir_all(&backup).unwrap();
        back_up(&root, &entries, &[], &backup, None, "2026-11-11T00:00:00Z").unwrap();
        assert!(apply(&entries, &backup).is_err());
        assert_eq!(fs::read(root.join("A/a.txt")).unwrap(), b"old a");
        assert!(!root.join("A/b.txt").exists());
        assert!(backup.join(ROLLED_BACK).is_file());
        fs::remove_dir_all(&base).unwrap();
    }
}
