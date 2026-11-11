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
mod clean;
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
    /// Absolute configured interpreter and archived study, exported to every
    /// child so build scripts never fall back to an unrelated PATH Python.
    child_env: Vec<(&'static str, PathBuf)>,
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
        let mut workspace = Self {
            root,
            config,
            package,
            child_env: Vec::new(),
        };
        for (name, key) in [
            ("C182_PYTHON", "python"),
            ("C182_LEGACY_STUDY", "legacy_study"),
        ] {
            if workspace.config.paths.contains_key(key) {
                // Absolute and normalized, without resolving junctions.
                let path = std::path::absolute(workspace.path(&workspace.config.paths, key)?)?;
                workspace.child_env.push((name, path));
            }
        }
        Ok(workspace)
    }
    fn configured_python(&self) -> Result<&Path> {
        self.child_env
            .iter()
            .find(|(name, _)| *name == "C182_PYTHON")
            .map(|(_, path)| path.as_path())
            .ok_or_else(|| error("Missing path setting python"))
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
            .envs(self.child_env.iter().map(|(k, v)| (k, v)))
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
        self.run(self.configured_python()?, &command)
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
        )?;
        self.python(
            "plugin_exports.py",
            &[self
                .root
                .join("out/cargo/release/c182_plugin.dll")
                .as_os_str()],
        )
    }
    fn cargo(&self, args: &[&str]) -> Result<()> {
        let cargo = env::var_os("CARGO").map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(env::var_os("USERPROFILE").unwrap_or_default())
                .join(".cargo/bin/cargo.exe")
        });
        self.run(cargo, &args.iter().map(OsStr::new).collect::<Vec<_>>())
    }
    /// Prepare the pinned inputs and the C++ forwarding headers that the
    /// oracle's reference build includes; the production build skips them.
    fn prepare_oracle_headers(&self) -> Result<()> {
        self.python(
            "tools/prepare_bootstrap.py",
            &[OsStr::new("--forwarding-headers")],
        )
    }
    fn test_ports(&self) -> Result<()> {
        self.prepare_oracle_headers()?;
        self.python("test_plugin_exports.py", &[])?;
        for script in [
            "tools/test_export_rpn_bundle.py",
            "tools/test_markdown_links.py",
        ] {
            let tests = self.root.join(script);
            self.run(self.configured_python()?, &[tests.as_os_str()])?;
        }
        self.cargo(&[
            "test",
            "-p",
            "c182-models",
            "-p",
            "c182-aircraft",
            "-p",
            "c182-recovered",
            "--release",
            "--lib",
        ])?;
        self.cargo(&[
            "test",
            "-p",
            "c182-oracle-tests",
            "--lib",
            "--test",
            "flight_start_source",
            "--test",
            "interface_native",
            "--test",
            "interface_sdk",
            "--test",
            "golden",
            "--profile",
            "oracle",
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
            "xplane-gauge-xml",
            "-p",
            "xplane-plugin",
            "-p",
            "c182-ui",
            "-p",
            "xplane-egui",
            "--release",
        ])?;
        Ok(())
    }
    fn test_fast(&self) -> Result<()> {
        self.cargo(&[
            "test",
            "-p",
            "c182-models",
            "-p",
            "c182-aircraft",
            "-p",
            "c182-recovered",
            "--lib",
        ])?;
        self.cargo(&[
            "test",
            "-p",
            "c182-oracle-tests",
            "--no-default-features",
            "--test",
            "golden",
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
            "xplane-gauge-xml",
            "-p",
            "xplane-plugin",
            "-p",
            "c182-ui",
            "-p",
            "xplane-egui",
        ])
    }
    fn test_oracle(&self, component: &str) -> Result<()> {
        let target = match component {
            "engine" => "oracle_engine",
            "flight" => "oracle_flight",
            "gauges" => "oracle_gauges",
            _ => return Err(error("Choose test-oracle engine, flight or gauges")),
        };
        self.prepare_oracle_headers()?;
        self.cargo(&[
            "test",
            "-p",
            "c182-oracle-tests",
            "--test",
            target,
            "--profile",
            "oracle",
            "--",
            "--nocapture",
        ])
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
                "source-runtime/xml/code_c182t_r.xml",
                "source-runtime/xml/code_s.xml",
                "source-runtime/xml/keys.xml",
                "source-runtime/xml/controls.xml",
                "source-runtime/xml/maint_hangar.xml",
                "source-runtime/programs.rpn",
                "source-runtime/resource-1002.luac",
            ] {
                if !root.join(&variant.folder).join(relative).is_file() {
                    return Err(error(format!("{} is missing {relative}", variant.id)));
                }
            }
        }
        for folder in self
            .package
            .variants
            .iter()
            .map(|v| &v.folder)
            .collect::<BTreeSet<_>>()
        {
            self.python(
                "plugin_exports.py",
                &[root
                    .join(folder)
                    .join(&self.package.plugin.binary)
                    .as_os_str()],
            )?;
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
        Some("test-fast") if args.len() == 1 => workspace.test_fast(),
        Some("test-oracle") if args.len() == 2 => workspace.test_oracle(&args[1]),
        Some("package") if args.len() == 1 || args.len() == 2 && args[1] == "--fresh-state" => {
            workspace.package(args.len() == 2)
        }
        Some("verify") if args.len() == 1 => {
            workspace.verify(&workspace.output("packages")?)?;
            Ok(())
        }
        Some("clean") => {
            let mut options = clean::Options {
                apply: false,
                days: 3,
            };
            let mut rest = args[1..].iter();
            while let Some(arg) = rest.next() {
                match arg.as_str() {
                    "--apply" => options.apply = true,
                    "--days" => {
                        options.days = rest
                            .next()
                            .and_then(|v| v.parse().ok())
                            .ok_or_else(|| error("--days needs a whole number of days"))?
                    }
                    _ => return Err(error("Usage: c182 clean [--days N] [--apply]")),
                }
            }
            Ok(clean::clean(&workspace.root.join("out"), &options)?)
        }
        Some("status") if args.len() == 1 => {
            println!(
                "Repository: {}\nRust core: {}\nVariants: {}\nGit: {}",
                workspace.root.display(),
                c182_core::IMPLEMENTATION,
                workspace.package.variants.len(),
                workspace.git(&["status", "--short"])
            );
            for (name, path) in &workspace.child_env {
                println!("{name}: {}", path.display());
            }
            Ok(())
        }
        _ => Err(error(
            "Usage: c182 build | test | test-fast | test-oracle engine|flight|gauges | package [--fresh-state] | verify | clean [--days N] [--apply] | status",
        )),
    }
}
