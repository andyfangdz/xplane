use std::{
    env, fs,
    path::{Path, PathBuf},
};
use xphost_core::{AircraftProfile, Config};

pub const USAGE: &str = "xphost [serve] --pkg PACKAGE --acf FILE [options]\n\
  --xpl PATH --clock fast|real --seconds N --script FILE --out DIR\n\
  --dt SECONDS (default 1/60) --port N (default 8199) --selftest (serve only)\n\
  --datarefs PATH|none --commands PATH|none --navdat PATH|none (default none)\n\
  --profile generic|c182|FILE.json (default generic)\n\
  --debug-request NAME --debug-result NAME --flight-start cold|running\n\
  --lat D --lon D --elev M --heading D --magvar D (default zero)";
#[derive(Debug)]
pub struct Options {
    pub serve: bool,
    pub selftest: bool,
    pub port: u16,
    pub fast: bool,
    pub dt: f64,
    pub seconds: Option<f64>,
    pub script: Option<PathBuf>,
    pub out: PathBuf,
    pub navdat: Option<PathBuf>,
    pub running: bool,
    pub debug_request: String,
    pub debug_result: String,
    pub config: Config,
}
fn absolute(p: impl AsRef<Path>) -> Result<PathBuf, String> {
    std::path::absolute(p).map_err(|e| e.to_string())
}
fn file(p: impl AsRef<Path>) -> Result<PathBuf, String> {
    let p = absolute(p)?;
    if p.is_file() {
        Ok(p)
    } else {
        Err(format!("file not found: {}", p.display()))
    }
}
fn optional_file(value: &str) -> Result<Option<String>, String> {
    if value == "none" {
        Ok(None)
    } else {
        file(value).map(|p| Some(p.to_string_lossy().into_owned()))
    }
}
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut args = args.into_iter().peekable();
    let (mut serve, mut selftest) = (false, false);
    let mut port = 8199;
    let (mut pkg, mut acf, mut clock, mut xpl, mut script) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    let (mut out, mut datarefs, mut commands, mut navdat) = (
        "out".to_string(),
        "none".to_string(),
        "none".to_string(),
        "none".to_string(),
    );
    let (mut seconds, mut dt) = (None, 1. / 60.);
    let mut profile = "generic".to_string();
    let (mut lat, mut lon, mut elev, mut heading, mut magvar) = (0f64, 0f64, 0f64, 0f64, 0f64);
    let (mut flight_start, mut debug_request, mut debug_result) =
        ("cold".to_string(), String::new(), String::new());
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "serve" => {
                serve = true;
                continue;
            }
            "--selftest" => {
                selftest = true;
                continue;
            }
            _ => (),
        }
        let value = args
            .next()
            .filter(|s| !s.starts_with("--"))
            .ok_or_else(|| format!("missing value for {arg}"))?;
        let number = || {
            value
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("invalid number for {arg}"))
        };
        match arg.as_str() {
            "--pkg" => pkg = value,
            "--acf" => acf = value,
            "--clock" => clock = value,
            "--xpl" => xpl = value,
            "--script" => script = value,
            "--out" => out = value,
            "--datarefs" => datarefs = value,
            "--commands" => commands = value,
            "--navdat" => navdat = value,
            "--profile" => profile = value,
            "--flight-start" => flight_start = value,
            "--debug-request" => debug_request = value,
            "--debug-result" => debug_result = value,
            "--seconds" => {
                let n = number()?;
                if n < 0. {
                    return Err("seconds must be nonnegative".into());
                }
                seconds = Some(n);
            }
            "--dt" => dt = number()?,
            "--lat" => lat = number()?,
            "--lon" => lon = number()?,
            "--elev" => elev = number()?,
            "--heading" => heading = number()?,
            "--magvar" => magvar = number()?,
            "--port" => {
                let n = number()?;
                if !(1. ..=65535.).contains(&n) || n.fract() != 0. {
                    return Err("port must be an integer in 1..65535".into());
                }
                port = n as u16;
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if pkg.is_empty() || acf.is_empty() || out.is_empty() {
        return Err("--pkg and --acf are required; --out must not be empty".into());
    }
    if dt <= 0. || dt > 1. || lat.abs() >= 90. || lon.abs() > 180. {
        return Err("dt must be in (0,1], latitude in (-90,90), longitude in [-180,180]".into());
    }
    if selftest && !serve
        || debug_request.is_empty() != debug_result.is_empty()
        || !matches!(flight_start.as_str(), "cold" | "running")
    {
        return Err("selftest requires serve; debug refs must be paired; flight-start must be cold or running".into());
    }
    if clock.is_empty() {
        clock = if serve { "real" } else { "fast" }.into();
    }
    if !matches!(clock.as_str(), "fast" | "real") {
        return Err("clock must be fast or real".into());
    }
    let pkg = absolute(pkg)?;
    let acf = file(pkg.join(acf))?;
    let xpl = if xpl.is_empty() {
        let mut candidates = Vec::new();
        if let Ok(dirs) = fs::read_dir(pkg.join("plugins")) {
            for d in dirs {
                let p = d.map_err(|e| e.to_string())?.path().join("64/win.xpl");
                if p.is_file() {
                    candidates.push(p);
                }
            }
        }
        candidates.sort();
        if candidates.len() != 1 {
            return Err(format!(
                "found {} plugins; select one with --xpl",
                candidates.len()
            ));
        }
        candidates.remove(0)
    } else {
        file(xpl)?
    };
    let datarefs = optional_file(&datarefs)?;
    let commands = optional_file(&commands)?;
    let navdat = optional_file(&navdat)?.map(PathBuf::from);
    let out = absolute(out)?;
    let system = out.join("xplane");
    let profile = if matches!(profile.as_str(), "generic" | "c182") {
        AircraftProfile::builtin(&profile)?
    } else {
        let path = file(&profile)?;
        AircraftProfile::from_json(
            &fs::read_to_string(&path)
                .map_err(|e| format!("cannot read aircraft profile {}: {e}", path.display()))?,
        )?
    };
    let config = Config {
        profile,
        out_dir: out.to_string_lossy().into_owned(),
        system_path: system.to_string_lossy().into_owned(),
        plugin_path: xpl.to_string_lossy().into_owned(),
        acf_path: acf.to_string_lossy().into_owned(),
        acf_name: acf.file_name().unwrap().to_string_lossy().into_owned(),
        datarefs,
        commands,
        lat,
        lon,
        elev,
        heading,
        magvar,
    };
    Ok(Some(Options {
        serve,
        selftest,
        port,
        fast: clock == "fast",
        dt,
        seconds,
        script: if script.is_empty() {
            None
        } else {
            Some(absolute(script)?)
        },
        out,
        navdat,
        running: flight_start == "running",
        debug_request,
        debug_result,
        config,
    }))
}
pub fn prepare(options: &Options) -> Result<(), String> {
    let root = Path::new(&options.config.system_path);
    for dir in [
        options.out.clone(),
        root.join("Output"),
        root.join("Resources/plugins"),
    ] {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let dest = root.join("Resources/default data/earth_nav.dat");
    if let Some(src) = &options.navdat {
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        if fs::canonicalize(src).ok() != fs::canonicalize(&dest).ok() {
            fs::copy(src, &dest).map_err(|e| e.to_string())?;
        }
    } else {
        match fs::remove_file(dest) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
pub fn from_env() -> Result<Option<Options>, String> {
    parse(env::args().skip(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_numbers_before_loading_plugin() {
        for n in ["0", "-1", "NaN", "inf", "junk"] {
            let args = ["--pkg", ".", "--acf", "dummy.acf", "--dt", n].map(str::to_owned);
            assert!(parse(args).is_err());
        }
    }
}
