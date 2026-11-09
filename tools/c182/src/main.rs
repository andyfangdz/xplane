use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
struct Config {
    paths: BTreeMap<String, String>,
    outputs: BTreeMap<String, String>,
}
#[derive(Deserialize)]
struct Package {
    variants: Vec<Variant>,
    plugin: Plugin,
}
#[derive(Deserialize)]
struct Variant {
    id: String,
    folder: String,
    acf: String,
}
#[derive(Deserialize)]
struct Plugin {
    binary: String,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    implementation: String,
    source_revision: String,
    source_status: String,
    #[serde(default)]
    public_revision: String,
    #[serde(default)]
    public_status: String,
    brackets: BTreeMap<String, String>,
    files: BTreeMap<String, String>,
}
struct Workspace {
    root: PathBuf,
    config: Config,
    package: Package,
}

fn ordinary_path(path: PathBuf) -> PathBuf {
    // Python/Pillow join OBJ texture paths containing '..'. Win32 verbatim
    // paths disable that normalization, so do not pass a \\?\ prefix across it.
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
}

fn error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    io::Error::other(message.into()).into()
}
fn hash(path: &Path) -> Result<String> {
    let mut input = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn is_link(path: &Path) -> Result<bool> {
    let info = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(info.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(windows))]
    {
        Ok(info.file_type().is_symlink())
    }
}
fn include_directories(path: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    output.push(path.to_owned());
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            include_directories(&entry.path(), output)?;
        }
    }
    Ok(())
}
fn walk(root: &Path, relative: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(root.join(relative))?.collect::<io::Result<_>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if is_link(&entry.path())? {
            return Err(error(format!(
                "Package contains a link: {}",
                entry.path().display()
            )));
        }
        let rel = relative.join(entry.file_name());
        if rel.components().any(|v| v.as_os_str() == "source-state") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            walk(root, &rel, files)?;
        } else if rel != Path::new("private-manifest.json") {
            files.insert(
                rel.to_string_lossy().replace('\\', "/"),
                hash(&entry.path())?,
            );
        }
    }
    Ok(())
}
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    if is_link(source)? {
        return Err(error("State trees must contain real files, not junctions"));
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if is_link(&entry.path())? {
            return Err(error(format!(
                "Linked state input: {}",
                entry.path().display()
            )));
        }
        let dest = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}
impl Workspace {
    fn load() -> Result<Self> {
        let root = ordinary_path(
            env::var_os("C182_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or(env::current_dir()?)
                .canonicalize()?,
        );
        if !root.join("package.toml").is_file() {
            return Err(error(
                "Set C182_WORKSPACE to the local aircraft workspace containing package.toml",
            ));
        }
        let config = toml::from_str(&fs::read_to_string(root.join("c182.local.toml"))?)?;
        let package: Package = toml::from_str(&fs::read_to_string(root.join("package.toml"))?)?;
        let mut ids = BTreeSet::new();
        for variant in &package.variants {
            if !ids.insert(&variant.id) {
                return Err(error("Duplicate aircraft variant"));
            }
            for value in [&variant.folder, &variant.acf] {
                if Path::new(value).components().count() != 1 {
                    return Err(error(
                        "Aircraft names must be single relative path components",
                    ));
                }
            }
        }
        Ok(Self {
            root,
            config,
            package,
        })
    }
    fn path(&self, section: &BTreeMap<String, String>, key: &str) -> Result<PathBuf> {
        let path = PathBuf::from(
            section
                .get(key)
                .ok_or_else(|| error(format!("Missing path setting {key}")))?,
        );
        Ok(if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        })
    }
    fn output(&self, key: &str) -> Result<PathBuf> {
        let path = self.path(&self.config.outputs, key)?;
        if path
            .components()
            .any(|v| matches!(v, std::path::Component::ParentDir))
        {
            return Err(error(
                "Output paths cannot contain parent-directory traversal",
            ));
        }
        if !path.starts_with(self.root.join("out")) {
            return Err(error(
                "Package outputs must stay under this checkout's out directory",
            ));
        }
        for ancestor in path
            .ancestors()
            .take_while(|p| p.starts_with(self.root.join("out")))
        {
            if ancestor.exists() && is_link(ancestor)? {
                return Err(error("Output paths cannot traverse directory links"));
            }
        }
        fs::create_dir_all(&path)?;
        let path = ordinary_path(path.canonicalize()?);
        let out = self.root.join("out");
        if !path.starts_with(&out) {
            return Err(error(format!(
                "{key} must stay under this checkout's out directory"
            )));
        }
        Ok(path)
    }
    fn run(&self, executable: impl AsRef<OsStr>, args: &[&OsStr]) -> Result<()> {
        let status = Command::new(executable)
            .args(args)
            .current_dir(&self.root)
            .env("C182_WORKSPACE", &self.root)
            .env("CARGO_TARGET_DIR", self.root.join("out/cargo"))
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(error(format!("Command failed with {status}")))
        }
    }
    fn python(&self, relative: &str, args: &[&OsStr]) -> Result<()> {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("python")
            .join(Path::new(relative).file_name().unwrap());
        let mut command = vec![OsStr::new("-I"), script.as_os_str()];
        command.extend_from_slice(args);
        self.run(self.path(&self.config.paths, "python")?, &command)
    }
    fn git(&self, args: &[&str]) -> String {
        self.git_at(&self.root, args)
    }
    fn git_at(&self, root: &Path, args: &[&str]) -> String {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    }
    fn build(&self) -> Result<()> {
        self.python("tools/prepare_bootstrap.py", &[])?;
        let cargo = env::var_os("CARGO").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(env::var_os("USERPROFILE").unwrap_or_default())
                .join(".cargo/bin/cargo.exe")
        });
        self.run(
            cargo,
            &[
                OsStr::new("build"),
                OsStr::new("--release"),
                OsStr::new("-p"),
                OsStr::new("c182-plugin"),
                OsStr::new("-j"),
                OsStr::new("4"),
            ],
        )
    }
    fn cargo(&self, args: &[&str]) -> Result<()> {
        let cargo = env::var_os("CARGO").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(env::var_os("USERPROFILE").unwrap_or_default())
                .join(".cargo/bin/cargo.exe")
        });
        self.run(cargo, &args.iter().map(OsStr::new).collect::<Vec<_>>())
    }
    fn test_ports(&self) -> Result<()> {
        self.cargo(&[
            "test",
            "-p",
            "c182-oracle-tests",
            "--release",
            "--",
            "--nocapture",
        ])?;
        self.cargo_public(&[
            "test",
            "-p",
            "c182-core",
            "-p",
            "xplane-numerics",
            "-p",
            "xplane-rpn",
            "-p",
            "c182-ui",
            "-p",
            "xplane-egui",
            "--release",
        ])?;
        self.cargo(&["build", "-p", "c182-aircraft", "--release"])?;
        let compiler = cc::windows_registry::find_tool("x86_64-pc-windows-msvc", "cl.exe")
            .ok_or_else(|| error("MSVC x64 compiler is unavailable"))?;
        let out = self.root.join("out/port-tests");
        fs::create_dir_all(&out)?;
        let sdk = self.path(&self.config.paths, "xplane_sdk")?;
        let cases: &[(&str, &[&str])] = &[
            ("controls-rows", &["xplane-host/test_controls_rows.cpp"]),
            (
                "inspection-camera",
                &["xplane-host/test_source_inspection_camera.cpp"],
            ),
            (
                "notification-ui",
                &[
                    "xplane-host/test_target_ui.cpp",
                    "xplane-host/target_ui.cpp",
                ],
            ),
            (
                "panel-cadence",
                &[
                    "xplane-host/test_panel_cadence.cpp",
                    "xplane-host/panel_cadence.cpp",
                ],
            ),
            ("radio-power", &["xplane-host/test_radio_power.cpp"]),
            (
                "poh-calibration",
                &[
                    "xplane-host/test_poh_calibration.cpp",
                    "xplane-host/poh_calibration.cpp",
                    "air-model/frame/propulsion_adapter.cpp",
                    "../tests/native/poh_hook_stubs.cpp",
                ],
            ),
            (
                "world-access",
                &[
                    "xplane-host/test_world.cpp",
                    "xplane-host/world_access.cpp",
                    "xplane-host/coordinates.cpp",
                ],
            ),
            (
                "persistence",
                &[
                    "persistence/codec_smoke.cpp",
                    "persistence/c182_state_codec.cpp",
                ],
            ),
            (
                "random-lifecycle",
                &[
                    "../tests/native/random_lifecycle.cpp",
                    "xplane-host/physical_random.cpp",
                ],
            ),
            (
                "standby-battery",
                &[
                    "xplane-host/test_standby_battery.cpp",
                    "xplane-host/standby_battery.cpp",
                ],
            ),
        ];
        let cpp_support = self.root.join("tests/support/retired-aircraft-src");
        let cpp_support = if cpp_support.is_dir() {
            cpp_support
        } else {
            self.root.join("src")
        };
        for &(name, sources) in cases {
            let directory = out.join(name);
            fs::create_dir_all(&directory)?;
            let exe = directory.join("check.exe");
            let mut command = compiler.to_command();
            command.current_dir(&directory).args([
                "/nologo",
                "/std:c++17",
                "/EHsc",
                "/O2",
                "/fp:strict",
                "/MD",
                "/W4",
                "/WX",
                "/DNOMINMAX=",
                "/D_CRT_SECURE_NO_WARNINGS",
                "/DIBM=1",
                "/DAPL=0",
                "/DLIN=0",
                "/DXPLM=1",
                "/DXPLM200=1",
                "/DXPLM210=1",
                "/DXPLM300=1",
                "/DXPLM301=1",
                "/DXPLM302=1",
                "/DXPLM400=1",
                "/DXPLM410=1",
            ]);
            let mut includes = vec![
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/c182-ui/include"),
                cpp_support.clone(),
                self.root.join("generated/bootstrap/xplane-host"),
                self.root.join("generated/bootstrap"),
                cpp_support.join("lua-host/native"),
                sdk.join("CHeaders/XPLM"),
                sdk.join("CHeaders/Widgets"),
            ];
            include_directories(&self.root.join("generated/bootstrap"), &mut includes)?;
            for include in includes {
                command.arg(format!("/I{}", include.display()));
            }
            for source in sources {
                let source = if let Some(relative) = source.strip_prefix("../tests/") {
                    self.root.join("tests").join(relative)
                } else {
                    cpp_support.join(source)
                };
                command.arg(source);
            }
            // Headers can initialize metadata through the runtime owner even
            // in otherwise pure checks. This archive includes models and core.
            command.arg(self.root.join("tests/native/offline_sdk.cpp"));
            command.arg(self.root.join("out/cargo/release/c182_aircraft.lib"));
            command.args([
                "kernel32.lib",
                "ntdll.lib",
                "userenv.lib",
                "ws2_32.lib",
                "dbghelp.lib",
                "bcrypt.lib",
                "ole32.lib",
                "user32.lib",
                "gdi32.lib",
                "gdiplus.lib",
                "opengl32.lib",
                "winmm.lib",
            ]);
            command.arg(format!("/Fe:{}", exe.display()));
            let output = command.output()?;
            let mut log = output.stdout.clone();
            log.extend_from_slice(&output.stderr);
            fs::write(directory.join("compile.log"), &log)?;
            if !output.status.success() {
                return Err(error(format!(
                    "{name} compilation failed:\n{}",
                    String::from_utf8_lossy(&log)
                )));
            }
            let mut command = Command::new(&exe);
            command.current_dir(&directory);
            if name == "world-access" {
                command.arg(
                    self.path(&self.config.paths, "legacy_study")?
                        .join("xplane-host/world-capture-fixture.bin"),
                );
            }
            if name == "persistence" {
                let input = self
                    .path(&self.config.paths, "legacy_study")?
                    .join("persistence");
                let fixture: serde_json::Value = serde_json::from_str(&fs::read_to_string(
                    input.join("original-log-decoded.json"),
                )?)?;
                let mut bytes = vec![2, 0, 0, 0];
                for section in ["persistent_locals", "lua_load_log_inputs"] {
                    for word in fixture[section]
                        .as_array()
                        .ok_or_else(|| error("Missing persistence fixture words"))?
                    {
                        let hex = word["raw_le_hex"]
                            .as_str()
                            .ok_or_else(|| error("Missing raw fixture word"))?;
                        for index in (0..hex.len()).step_by(2) {
                            bytes.push(u8::from_str_radix(&hex[index..index + 2], 16)?);
                        }
                    }
                }
                let fixture_file = directory.join("reference-log.bin");
                fs::write(&fixture_file, bytes)?;
                if hash(&fixture_file)? != fixture["sha256"].as_str().unwrap_or_default() {
                    return Err(error("Persistence fixture reconstruction changed its hash"));
                }
                command
                    .arg(fixture_file)
                    .arg(input.join("fixture-names.txt"));
            }
            let output = command.output()?;
            let mut log = output.stdout.clone();
            log.extend_from_slice(&output.stderr);
            fs::write(directory.join("test.log"), &log)?;
            print!("{}", String::from_utf8_lossy(&log));
            if !output.status.success() {
                return Err(error(format!("{name} failed with {}", output.status)));
            }
        }
        println!("All {} native adapter suites passed.", cases.len());
        Ok(())
    }
    fn cargo_public(&self, args: &[&str]) -> Result<()> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Cargo.toml");
        let path = manifest.to_string_lossy();
        let mut arguments = args.to_vec();
        arguments.extend(["--manifest-path", path.as_ref()]);
        self.cargo(&arguments)
    }
    fn verify(&self, root: &Path) -> Result<usize> {
        let manifest: Manifest =
            serde_json::from_str(&fs::read_to_string(root.join("private-manifest.json"))?)?;
        let mut files = BTreeMap::new();
        walk(root, Path::new(""), &mut files)?;
        if files != manifest.files {
            return Err(error("Package file set or hashes differ from its manifest"));
        }
        for variant in &self.package.variants {
            for relative in [
                &variant.acf,
                &self.package.plugin.binary,
                "source-runtime/gauge-rpn_vm.lua",
                "source-runtime/resource-1002.luac",
            ] {
                if !root.join(&variant.folder).join(relative).is_file() {
                    return Err(error(format!("{} is missing {relative}", variant.id)));
                }
            }
        }
        println!(
            "Verified {} immutable package files across {} aircraft variants",
            files.len(),
            self.package.variants.len()
        );
        Ok(files.len())
    }
    fn package(&self, fresh: bool) -> Result<()> {
        let output = self.output("packages")?;
        let staging_parent = self.root.join("out/staging");
        fs::create_dir_all(&staging_parent)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            .to_string();
        let staging = staging_parent.join(&stamp);
        fs::create_dir(&staging)?;
        {
            for folder in self
                .package
                .variants
                .iter()
                .map(|v| &v.folder)
                .collect::<BTreeSet<_>>()
            {
                let source = output.join(folder).join("source-state");
                if source.is_dir() {
                    copy_tree(&source, &staging.join(folder).join("source-state"))?;
                }
            }
        }
        let main = staging.join("C182_Converted");
        let mut args = vec![
            OsStr::new("--target"),
            main.as_os_str(),
            OsStr::new("--release"),
        ];
        if fresh {
            args.push(OsStr::new("--fresh-state"));
        }
        self.python("tools/assemble.py", &args)?;
        let mut files = BTreeMap::new();
        walk(&staging, Path::new(""), &mut files)?;
        let manifest = Manifest {
            schema: 1,
            implementation: c182_core::IMPLEMENTATION.to_owned(),
            source_revision: self.git(&["rev-parse", "HEAD"]),
            source_status: self.git(&["status", "--porcelain"]),
            public_revision: self.git_at(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &["rev-parse", "HEAD"],
            ),
            public_status: self.git_at(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &["status", "--porcelain"],
            ),
            brackets: env::vars()
                .filter(|(k, _)| k.starts_with("C182_BR_") || k == "C182_VISUAL_SKIP")
                .collect(),
            files,
        };
        fs::write(
            staging.join("private-manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        self.verify(&staging)?;
        // A verified package set is promoted as one directory. Failed assembly
        // leaves the current output and every installed aircraft untouched.
        let backup = staging_parent.join(format!("previous-{stamp}"));
        fs::rename(&output, &backup)?;
        if let Err(e) = fs::rename(&staging, &output) {
            fs::rename(&backup, &output)?;
            return Err(e.into());
        }
        println!("Package set: {}", output.display());
        println!("Previous output retained at {}", backup.display());
        Ok(())
    }
}
fn main() {
    if let Err(e) = execute() {
        eprintln!("c182: {e}");
        std::process::exit(1);
    }
}
fn execute() -> Result<()> {
    let workspace = Workspace::load()?;
    let args: Vec<_> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build") if args.len() == 1 => workspace.build(),
        Some("test") if args.len() == 1 => workspace.test_ports(),
        Some("package") if args.len() == 1 || args.len() == 2 && args[1] == "--fresh-state" => {
            workspace.package(args.len() == 2)
        }
        Some("verify") if args.len() == 1 => {
            workspace.verify(&workspace.output("packages")?)?;
            Ok(())
        }
        Some("status") if args.len() == 1 => {
            println!(
                "Repository: {}\nRust core: {}\nVariants: {}\nGit: {}",
                workspace.root.display(),
                c182_core::IMPLEMENTATION,
                workspace.package.variants.len(),
                workspace.git(&["status", "--short"])
            );
            Ok(())
        }
        _ => Err(error(
            "Usage: c182 build | test | package [--fresh-state] | verify | status",
        )),
    }
}
