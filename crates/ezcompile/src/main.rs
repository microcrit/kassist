use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cakewalk::{python_tool_api, python_tool_extensions};
use stonecutter::{
    python_component_adapter, python_krita_api, python_package_prelude, python_torch_api,
};

const ENVIRONMENT_STAMP: &str = ".kassist-environment";

#[derive(Clone, Debug, Eq, PartialEq)]
enum PythonRuntime {
    Host(PathBuf),
    Flatpak(String),
}

impl PythonRuntime {
    fn command(&self, interpreter: Option<&Path>) -> Command {
        match self {
            Self::Host(python) => Command::new(interpreter.unwrap_or(python)),
            Self::Flatpak(application) => {
                let executable = interpreter.unwrap_or_else(|| Path::new("python3"));
                let mut command = Command::new("flatpak");
                command
                    .arg("run")
                    .arg(format!("--command={}", executable.display()))
                    .arg(application);
                command
            }
        }
    }
}

fn main() {
    if let Err(error) = run(env::args().skip(1)) {
        eprintln!("ezcompile: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    let root = workspace_root();
    let options = Options::parse(arguments, &root)?;
    let packages = if options.packages.is_empty() {
        discover_plugins(&root)?
    } else {
        options.packages
    };

    if packages.is_empty() {
        return Err("no plugin packages were found".into());
    }

    let python = match options.python.as_ref() {
        Some(path) => PythonRuntime::Host(path.clone()),
        None if options.install_target == Some(InstallTarget::Flatpak) => {
            PythonRuntime::Flatpak(options.flatpak_id.clone())
        }
        None => PythonRuntime::Host(find_python()?),
    };

    for package in &packages {
        package_plugin(
            &root,
            &options.output,
            package,
            options.dev,
            &python,
        )?;
    }
    if let Some(target) = options.install_target {
        let resource_root =
            krita_resource_root(target, options.resource_dir.as_deref(), &options.flatpak_id)?;
        for package in &packages {
            install_plugin(&options.output, package, &resource_root)?;
        }
        println!("installed into {}", resource_root.join("pykrita").display());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallTarget {
    User,
    Flatpak,
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    output: PathBuf,
    packages: Vec<String>,
    dev: bool,
    python: Option<PathBuf>,
    install_target: Option<InstallTarget>,
    resource_dir: Option<PathBuf>,
    flatpak_id: String,
}

impl Options {
    fn parse(arguments: impl IntoIterator<Item = String>, root: &Path) -> Result<Self, String> {
        let mut output = root.join("out");
        let mut packages = Vec::new();
        let mut dev = false;
        let mut python = env::var_os("KASSIST_PYTHON").map(PathBuf::from);
        let mut install_target = None;
        let mut resource_dir = None;
        let mut flatpak_id = "org.kde.krita".to_owned();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "install" => {
                    if install_target.replace(InstallTarget::User).is_some() {
                        return Err("install mode was specified more than once".into());
                    }
                }
                "flatpak" => {
                    if arguments.next().as_deref() != Some("install") {
                        return Err("flatpak must be followed by install".into());
                    }
                    if install_target.replace(InstallTarget::Flatpak).is_some() {
                        return Err("install mode was specified more than once".into());
                    }
                }
                "--dev" => dev = true,
                "--output" | "-o" => {
                    let value = arguments.next().ok_or("--output requires a directory")?;
                    output = PathBuf::from(value);
                }
                "--python" => {
                    let value = arguments
                        .next()
                        .ok_or("--python requires an executable path")?;
                    python = Some(PathBuf::from(value));
                }
                "--resource-dir" => {
                    let value = arguments
                        .next()
                        .ok_or("--resource-dir requires a Krita resource directory")?;
                    resource_dir = Some(PathBuf::from(value));
                }
                "--flatpak-id" => {
                    let value = arguments
                        .next()
                        .ok_or("--flatpak-id requires an application ID")?;
                    if !valid_flatpak_id(&value) {
                        return Err(
                            "--flatpak-id must contain only letters, digits, '.', '_' or '-'"
                                .into(),
                        );
                    }
                    flatpak_id = value;
                }
                "--help" | "-h" => return Err(help_text().into()),
                value if value.starts_with('-') => {
                    return Err(format!("unknown option '{value}'\n\n{}", help_text()));
                }
                package => {
                    if !package
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                    {
                        return Err(format!("invalid package name '{package}'"));
                    }
                    packages.push(package.to_owned());
                }
            }
        }
        if flatpak_id != "org.kde.krita" && install_target != Some(InstallTarget::Flatpak) {
            return Err("--flatpak-id is only valid with flatpak install".into());
        }
        Ok(Self {
            output,
            packages,
            dev,
            python,
            install_target,
            resource_dir,
            flatpak_id,
        })
    }
}

fn help_text() -> &'static str {
    "Usage:\n  ezcompile [OPTIONS] [PLUGIN...]\n  ezcompile install [OPTIONS] [PLUGIN...]\n  ezcompile flatpak install [OPTIONS] [PLUGIN...]\n\nBuilds each named plugin in release mode and writes its self-contained Krita\nproject to OUTPUT/<plugin>/pykrita/. install also copies each desktop file and\nmodule folder into the current user's Krita resource directory; flatpak install\ntargets ~/.var/app/org.kde.krita/data/krita by default. Use --resource-dir DIR\nfor a custom Krita resource directory or --flatpak-id ID for another Flatpak\napplication ID. If a plugin has requirements.txt, ezcompile creates a dedicated\nvirtual environment, installs it, and copies its site-packages to the project's\nlib/ directory. That environment is reused until the requirements or the\n--python interpreter change, and uv replaces pip when it is on PATH.\n--python (or KASSIST_PYTHON) must be compatible with Krita's\nPython when embedding native wheels. With no plugin names, packages every crate\nunder plugins/. --dev bypasses license activation in the generated package."
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("ezcompile must live in crates/ezcompile")
        .to_owned()
}

fn discover_plugins(root: &Path) -> Result<Vec<String>, String> {
    let plugin_root = root.join("plugins");
    let mut packages = Vec::new();
    for entry in fs::read_dir(&plugin_root)
        .map_err(|error| format!("cannot read {}: {error}", plugin_root.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            packages.push(read_manifest(&manifest)?.name);
        }
    }
    packages.sort();
    Ok(packages)
}

fn package_plugin(
    root: &Path,
    output: &Path,
    package: &str,
    dev: bool,
    python: &PythonRuntime,
) -> Result<(), String> {
    let manifest_path = root.join("plugins").join(package).join("Cargo.toml");
    let manifest = read_manifest(&manifest_path)?;
    if manifest.name != package {
        return Err(format!(
            "package '{package}' must live in plugins/{package}; found '{}'",
            manifest.name
        ));
    }

    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "--release", "--lib", "--package", package])
        .status()
        .map_err(|error| format!("could not run cargo for '{package}': {error}"))?;
    if !status.success() {
        return Err(format!("cargo could not build plugin '{package}'"));
    }

    let library_name = shared_library_name(package);
    let built_library = root.join("target").join("release").join(&library_name);
    if !built_library.is_file() {
        return Err(format!(
            "cargo completed but {} was not produced; ensure the plugin has crate-type = [\"cdylib\", \"rlib\"]",
            built_library.display()
        ));
    }

    let package_dir = output.join(package);
    fs::create_dir_all(&package_dir)
        .map_err(|error| format!("cannot create {}: {error}", package_dir.display()))?;
    fs::copy(&built_library, package_dir.join(&library_name))
        .map_err(|error| format!("cannot copy '{}': {error}", built_library.display()))?;
    fs::write(
        package_dir.join("plugin.toml"),
        format!(
            "[plugin]\nname = \"{}\"\nversion = \"{}\"\nlibrary = \"{}\"\n",
            manifest.name, manifest.version, library_name
        ),
    )
    .map_err(|error| format!("cannot write package metadata: {error}"))?;
    write_krita_project(root, output, package, &manifest, &library_name, dev, python)?;
    println!("packaged {} -> {}", package, package_dir.display());
    Ok(())
}

fn install_plugin(output: &Path, package: &str, resource_root: &Path) -> Result<(), String> {
    let project = krita_project_root(output, package);
    let source_module = project.join(package);
    let source_desktop = project.join(format!("{package}.desktop"));
    if !source_module.is_dir() || !source_desktop.is_file() {
        return Err(format!(
            "{} is not a complete Krita project; package it before installing",
            project.display()
        ));
    }

    let destination = resource_root.join("pykrita");
    fs::create_dir_all(&destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    let staged_module = destination.join(format!(".{package}.ezcompile-staging"));
    if staged_module.exists() {
        fs::remove_dir_all(&staged_module)
            .map_err(|error| format!("cannot clear {}: {error}", staged_module.display()))?;
    }
    fs::create_dir_all(&staged_module).map_err(|error| error.to_string())?;
    copy_tree(&source_module, &staged_module)?;
    replace_directory(
        &staged_module,
        &destination.join(package),
        &destination.join(format!(".{package}.ezcompile-backup")),
    )?;

    let staged_desktop = destination.join(format!(".{package}.desktop.ezcompile-staging"));
    fs::copy(&source_desktop, &staged_desktop)
        .map_err(|error| format!("cannot stage {}: {error}", source_desktop.display()))?;
    replace_file(
        &staged_desktop,
        &destination.join(format!("{package}.desktop")),
        &destination.join(format!(".{package}.desktop.ezcompile-backup")),
    )?;
    println!("installed {} -> {}", package, destination.display());
    Ok(())
}

fn replace_directory(staged: &Path, target: &Path, backup: &Path) -> Result<(), String> {
    if backup.exists() {
        fs::remove_dir_all(backup)
            .map_err(|error| format!("cannot clear {}: {error}", backup.display()))?;
    }
    let had_target = target.exists();
    if had_target {
        fs::rename(target, backup)
            .map_err(|error| format!("cannot stage existing {}: {error}", target.display()))?;
    }
    if let Err(error) = fs::rename(staged, target) {
        if had_target {
            let _ = fs::rename(backup, target);
        }
        return Err(format!("cannot install {}: {error}", target.display()));
    }
    if had_target {
        fs::remove_dir_all(backup)
            .map_err(|error| format!("cannot clear backup {}: {error}", backup.display()))?;
    }
    Ok(())
}

fn replace_file(staged: &Path, target: &Path, backup: &Path) -> Result<(), String> {
    if backup.exists() {
        fs::remove_file(backup)
            .map_err(|error| format!("cannot clear {}: {error}", backup.display()))?;
    }
    let had_target = target.exists();
    if had_target {
        fs::rename(target, backup)
            .map_err(|error| format!("cannot stage existing {}: {error}", target.display()))?;
    }
    if let Err(error) = fs::rename(staged, target) {
        if had_target {
            let _ = fs::rename(backup, target);
        }
        return Err(format!("cannot install {}: {error}", target.display()));
    }
    if had_target {
        fs::remove_file(backup)
            .map_err(|error| format!("cannot clear backup {}: {error}", backup.display()))?;
    }
    Ok(())
}

fn krita_resource_root(
    target: InstallTarget,
    specified: Option<&Path>,
    flatpak_id: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = specified {
        return Ok(path.to_owned());
    }
    let home = user_home_directory()?;
    match target {
        InstallTarget::Flatpak => {
            if !cfg!(unix) {
                return Err("flatpak install is only supported on Unix-like hosts".into());
            }
            Ok(home
                .join(".var")
                .join("app")
                .join(flatpak_id)
                .join("data")
                .join("krita"))
        }
        InstallTarget::User => {
            #[cfg(target_os = "windows")]
            {
                return env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("krita"))
                    .ok_or_else(|| "APPDATA is required to locate Krita resources".into());
            }
            #[cfg(target_os = "macos")]
            {
                return Ok(home
                    .join("Library")
                    .join("Application Support")
                    .join("Krita"));
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let data_home = env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".local").join("share"));
                Ok(data_home.join("krita"))
            }
            #[cfg(not(any(unix, target_os = "windows")))]
            {
                let _ = home;
                Err("cannot determine the Krita resource directory on this platform; use --resource-dir".into())
            }
        }
    }
}

fn user_home_directory() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "HOME (or USERPROFILE) is required to locate Krita resources".into())
}

fn valid_flatpak_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn write_krita_project(
    root: &Path,
    output: &Path,
    package: &str,
    manifest: &PackageManifest,
    library_name: &str,
    dev: bool,
    python: &PythonRuntime,
) -> Result<(), String> {
    let pykrita = krita_project_root(output, package);
    let module = pykrita.join(package);
    if module.exists() {
        fs::remove_dir_all(&module)
            .map_err(|error| format!("cannot replace {}: {error}", module.display()))?;
    }
    fs::create_dir_all(module.join("native")).map_err(|error| error.to_string())?;
    fs::copy(
        output.join(package).join(library_name),
        module.join("native").join(library_name),
    )
    .map_err(|error| error.to_string())?;
    let source = root.join("plugins").join(package).join("python");
    if source.is_dir() {
        copy_plugin_source(&source, &module)?;
    }
    let main_file = module.join(format!("{package}.py"));
    if !main_file.exists() {
        fs::write(&main_file, "from krita import Extension, Krita\n\nclass KassistExtension(Extension):\n    def setup(self): pass\n    def createActions(self, window): pass\n\nKrita.instance().addExtension(KassistExtension(Krita.instance()))\n").map_err(|error| error.to_string())?;
    }
    fs::write(module.join("kassist_prelude.py"), python_package_prelude())
        .map_err(|error| error.to_string())?;
    fs::write(
        module.join("kassist_components.py"),
        python_component_adapter(),
    )
    .map_err(|error| error.to_string())?;
    fs::write(module.join("kassist_krita.py"), python_krita_api())
        .map_err(|error| error.to_string())?;
    fs::write(module.join("kassist_torch.py"), python_torch_api())
        .map_err(|error| error.to_string())?;
    fs::write(module.join("kassist_cakewalk.py"), python_tool_api())
        .map_err(|error| error.to_string())?;
    for (name, source) in python_tool_extensions() {
        let path = module.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, source).map_err(|error| error.to_string())?;
    }
    fs::write(
        module.join("__init__.py"),
        format!(
            "# Generated bridge entrypoint.\nfrom .kassist_prelude import prepare_environment as _prepare_environment\n_prepare_environment()\ndel _prepare_environment\nfrom .{package} import *\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    fs::write(
        pykrita.join(format!("{package}.desktop")),
        format!("[Desktop Entry]\nType=Service\nServiceTypes=Krita/PythonPlugin\nX-KDE-Library={package}\nX-Python-2-Compatible=false\nName={}\nComment=kassist plugin {}\n", title_case(package), manifest.version),
    ).map_err(|error| error.to_string())?;
    fs::write(module.join("kassist_runtime.py"), format!("DEV_MODE = {}\nPLUGIN_ID = 'cc.hellings.{}'\nGUMROAD_PRODUCT_PERMALINK = ''\n", if dev { "True" } else { "False" }, package)).map_err(|error| error.to_string())?;
    fs::write(
        module.join("license.py"),
        r#"import json
import os
from urllib.parse import urlencode
from urllib.request import urlopen

from .kassist_runtime import GUMROAD_PRODUCT_PERMALINK, PLUGIN_ID

_SETTING_KEY = "kassist/donations/" + PLUGIN_ID

def _settings():
    from PyQt5.QtCore import QSettings
    return QSettings("Hellings", "kassist")

def donation_key():
    return os.environ.get("KASSIST_GUMROAD_KEY") or _settings().value(_SETTING_KEY, "", type=str)

def verify_donation(key):
    if not GUMROAD_PRODUCT_PERMALINK:
        return False, "Donation verification is not configured for this plugin."
    query = urlencode({"product_permalink": GUMROAD_PRODUCT_PERMALINK, "license_key": key, "increment_uses_count": "false"})
    try:
        with urlopen("https://api.gumroad.com/v2/licenses/verify?" + query, timeout=5) as response:
            payload = json.load(response)
    except Exception as error:
        return False, "Could not verify the Gumroad receipt: {}".format(error)
    return bool(payload.get("success")), payload.get("message", "Thanks for supporting {}!".format(PLUGIN_ID))

def activate(key):
    key = key.strip()
    if not key:
        return False, "Enter a Gumroad receipt key, or use the plugin freely."
    verified, message = verify_donation(key)
    if verified:
        _settings().setValue(_SETTING_KEY, key)
    return verified, message

def forget_license():
    _settings().remove(_SETTING_KEY)

def is_licensed():
    return True
"#,
    )
    .map_err(|error| error.to_string())?;
    embed_python_requirements(root, output, package, &module, python)?;
    Ok(())
}

fn copy_plugin_source(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry.file_name() == "__init__.py" || entry.file_name() == "__pycache__" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            fs::create_dir_all(&target).map_err(|error| error.to_string())?;
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn embed_python_requirements(
    root: &Path,
    output: &Path,
    package: &str,
    module: &Path,
    python: &PythonRuntime,
) -> Result<(), String> {
    let source_requirements = root.join("plugins").join(package).join("requirements.txt");
    let packaged_requirements = module.join("requirements.txt");
    let requirements = if source_requirements.is_file() {
        let contents = fs::read_to_string(&source_requirements)
            .map_err(|error| format!("cannot read {}: {error}", source_requirements.display()))?;
        fs::copy(&source_requirements, &packaged_requirements)
            .map_err(|error| format!("cannot copy {}: {error}", source_requirements.display()))?;
        contents
    } else {
        fs::write(
            &packaged_requirements,
            "# No external Python dependencies.\n",
        )
        .map_err(|error| error.to_string())?;
        String::new()
    };

    let venv = output.join(".venv").join(package);
    if !requirements_have_packages(&requirements) {
        discard_environment(&venv)?;
        fs::create_dir_all(module.join("lib")).map_err(|error| error.to_string())?;
        return Ok(());
    }

    let library = module.join("lib");
    if library.exists() {
        fs::remove_dir_all(&library)
            .map_err(|error| format!("cannot replace {}: {error}", library.display()))?;
    }
    if let PythonRuntime::Host(path) = python {
        if !path.is_file() && path.components().count() > 1 {
            return Err(format!("Python executable {} does not exist", path.display()));
        }
    }
    let stamp = format!(
        "{}\n{}\n",
        content_digest(&requirements),
        python_identity(python)?
    );
    if environment_is_current(&venv, &stamp) {
        println!("reused Python environment {}", venv.display());
    } else {
        create_environment(python, &venv, &packaged_requirements, package)?;
        fs::write(venv.join(ENVIRONMENT_STAMP), &stamp)
            .map_err(|error| format!("cannot record {}: {error}", venv.display()))?;
    }
    let venv_python = venv_python(&venv);
    verify_accelerator_runtime(package, python, &venv_python)?;
    let site_packages = python_site_packages(python, &venv_python)?;
    fs::create_dir_all(&library).map_err(|error| error.to_string())?;
    copy_tree(&site_packages, &library)?;
    Ok(())
}

fn create_environment(
    python: &PythonRuntime,
    venv: &Path,
    requirements: &Path,
    package: &str,
) -> Result<(), String> {
    discard_environment(venv)?;
    let uv = find_uv();
    let use_uv = uv.is_some() && matches!(python, PythonRuntime::Host(_));
    let mut create = python.command(None);
    create.args(["-m", "venv"]);
    if use_uv {
        create.arg("--without-pip");
    }
    run_command(create.arg(venv), "create Python virtual environment")?;
    let interpreter = venv_python(venv);
    println!(
        "installing {package} requirements into {} with {}",
        venv.display(),
        installer_name(use_uv)
    );
    let mut install = match (&uv, python) {
        (Some(uv), PythonRuntime::Host(_)) => {
            let mut install = Command::new(uv);
            install
                .args(["pip", "install", "--python"])
                .arg(&interpreter);
            install
        }
        _ => {
            let mut install = python.command(Some(&interpreter));
            install.args(["-m", "pip", "install", "--disable-pip-version-check"]);
            install
        }
    };
    run_command(
        install.arg("--requirement").arg(requirements),
        "install Python requirements",
    )
}

fn installer_name(uv: bool) -> &'static str {
    if uv { "uv" } else { "pip" }
}

fn environment_is_current(venv: &Path, stamp: &str) -> bool {
    venv_python(venv).is_file()
        && fs::read_to_string(venv.join(ENVIRONMENT_STAMP)).is_ok_and(|current| current == stamp)
}

fn discard_environment(venv: &Path) -> Result<(), String> {
    if venv.exists() {
        fs::remove_dir_all(venv)
            .map_err(|error| format!("cannot replace {}: {error}", venv.display()))?;
    }
    Ok(())
}

fn requirements_have_packages(requirements: &str) -> bool {
    requirements.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with('#')
    })
}

fn python_identity(python: &PythonRuntime) -> Result<String, String> {
    let output = python
        .command(None)
        .args([
            "-c",
            "import sys; print(sys.executable, sys.version.split()[0])",
        ])
        .output()
        .map_err(|error| format!("cannot inspect Python runtime: {error}"))?;
    if !output.status.success() {
        return Err("Python runtime could not report its version".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn content_digest(contents: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in contents.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn find_python() -> Result<PathBuf, String> {
    for candidate in ["python3", "python"] {
        let status = Command::new(candidate).arg("--version").status();
        if matches!(status, Ok(status) if status.success()) {
            return Ok(PathBuf::from(candidate));
        }
    }
    Err("Python 3 is required to embed requirements; install python3 with venv support".into())
}

fn find_uv() -> Option<PathBuf> {
    let status = Command::new("uv").arg("--version").output();
    matches!(status, Ok(output) if output.status.success()).then(|| PathBuf::from("uv"))
}

fn verify_accelerator_runtime(
    package: &str,
    runtime: &PythonRuntime,
    python: &Path,
) -> Result<(), String> {
    if package != "shadeproject" {
        return Ok(());
    }
    let output = runtime
        .command(Some(python))
        .args([
            "-c",
            "import torch; assert torch.version.cuda, 'CUDA-enabled PyTorch is required'; print(torch.version.cuda)",
        ])
        .output()
        .map_err(|error| format!("cannot verify CUDA PyTorch runtime: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let details = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "ShadeProject requires a CUDA-enabled PyTorch wheel; verification failed: {}",
            details.trim()
        ))
    }
}

fn venv_python(venv: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    }
}

fn python_site_packages(runtime: &PythonRuntime, python: &Path) -> Result<PathBuf, String> {
    let output = runtime
        .command(Some(python))
        .args(["-c", "import site; print(site.getsitepackages()[0])"])
        .output()
        .map_err(|error| format!("cannot locate site-packages: {error}"))?;
    if !output.status.success() {
        return Err("Python could not locate its site-packages directory".into());
    }
    let path = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_owned();
    if path.is_empty() {
        return Err("Python reported an empty site-packages path".into());
    }
    Ok(PathBuf::from(path))
}

fn run_command(command: &mut Command, description: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("could not {description}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let mut details = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if details.is_empty() {
            details = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        }
        let tail = details
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>();
        Err(format!(
            "could not {description}{}",
            if tail.is_empty() {
                String::new()
            } else {
                format!(": {}", tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
            }
        ))
    }
}

fn title_case(value: &str) -> String {
    value
        .split('-')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

struct PackageManifest {
    name: String,
    version: String,
}

fn read_manifest(path: &Path) -> Result<PackageManifest, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let name = toml_string(&contents, "name")
        .ok_or_else(|| format!("{} has no package name", path.display()))?;
    let version = toml_string(&contents, "version")
        .ok_or_else(|| format!("{} has no package version", path.display()))?;
    Ok(PackageManifest { name, version })
}

fn toml_string(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (found, value) = line.split_once('=')?;
        (found.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
    })
}

fn shared_library_name(package: &str) -> String {
    let stem = package.replace('-', "_");
    if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

fn krita_project_root(output: &Path, package: &str) -> PathBuf {
    output.join(package).join("pykrita")
}

#[cfg(test)]
mod tests {
    use super::{
        ENVIRONMENT_STAMP, InstallTarget, Options, PythonRuntime, content_digest, environment_is_current,
        find_python, install_plugin, krita_project_root, python_identity,
        requirements_have_packages, shared_library_name, toml_string, venv_python,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn options_accept_plugin_names_and_an_output_directory() {
        let options = Options::parse(
            ["--output".into(), "dist".into(), "threedee".into()],
            Path::new("."),
        )
        .unwrap();
        assert_eq!(options.packages, ["threedee"]);
        assert_eq!(options.output, Path::new("dist"));
    }

    #[test]
    fn options_accept_both_install_command_forms() {
        let normal = Options::parse(["install".into(), "pinwheel".into()], Path::new(".")).unwrap();
        assert_eq!(normal.install_target, Some(InstallTarget::User));

        let flatpak = Options::parse(
            [
                "flatpak".into(),
                "install".into(),
                "--flatpak-id".into(),
                "org.kde.krita".into(),
                "simassist".into(),
            ],
            Path::new("."),
        )
        .unwrap();
        assert_eq!(flatpak.install_target, Some(InstallTarget::Flatpak));
        assert_eq!(flatpak.packages, ["simassist"]);
    }

    #[test]
    fn manifest_values_are_read_from_simple_package_metadata() {
        let manifest = "[package]\nname = \"threedee\"\nversion = \"0.1.0\"";
        assert_eq!(toml_string(manifest, "name"), Some("threedee".into()));
    }

    #[test]
    fn generated_library_name_has_a_platform_extension() {
        assert!(shared_library_name("shade-project").contains("shade_project"));
    }

    #[test]
    fn krita_files_are_scoped_to_their_plugin_package() {
        assert_eq!(
            krita_project_root(Path::new("dist"), "pinwheel"),
            Path::new("dist").join("pinwheel").join("pykrita")
        );
    }

    #[test]
    fn generated_license_helper_preserves_server_rejection_messages() {
        let source = include_str!("main.rs");
        assert!(source.contains("api.gumroad.com/v2/licenses/verify"));
        assert!(source.contains("return True"));
        assert!(source.contains("kassist_cakewalk.py"));
    }

    #[test]
    fn install_replaces_only_a_plugin_pair_in_the_resource_folder() {
        let root = temporary_directory("install-test");
        let output = root.join("out");
        let project = krita_project_root(&output, "pinwheel");
        let source_module = project.join("pinwheel");
        fs::create_dir_all(&source_module).unwrap();
        fs::write(project.join("pinwheel.desktop"), "desktop-v1").unwrap();
        fs::write(source_module.join("pinwheel.py"), "source-v1").unwrap();
        let resources = root.join("resources");

        install_plugin(&output, "pinwheel", &resources).unwrap();
        let installed = resources.join("pykrita").join("pinwheel");
        assert_eq!(
            fs::read_to_string(installed.join("pinwheel.py")).unwrap(),
            "source-v1"
        );
        assert_eq!(
            fs::read_to_string(resources.join("pykrita").join("pinwheel.desktop")).unwrap(),
            "desktop-v1"
        );

        fs::write(source_module.join("pinwheel.py"), "source-v2").unwrap();
        fs::write(installed.join("obsolete.py"), "old").unwrap();
        install_plugin(&output, "pinwheel", &resources).unwrap();
        assert_eq!(
            fs::read_to_string(installed.join("pinwheel.py")).unwrap(),
            "source-v2"
        );
        assert!(!installed.join("obsolete.py").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn requirements_ignore_comments_and_blank_lines() {
        assert!(!requirements_have_packages(""));
        assert!(!requirements_have_packages("# CUDA 12.8 wheels\n\n"));
        assert!(requirements_have_packages("# comment\n\n  torch>=2.5\n"));
    }

    #[test]
    fn environment_is_reused_until_requirements_or_interpreter_change() {
        let root = temporary_directory("environment-test");
        let venv = root.join(".venv").join("pinwheel");
        fs::create_dir_all(venv_python(&venv).parent().unwrap()).unwrap();
        fs::write(venv_python(&venv), "#!/bin/sh\n").unwrap();
        let stamp = format!(
            "{}\n/usr/bin/python3 3.12.1\n",
            content_digest("torch>=2.5")
        );
        assert!(!environment_is_current(&venv, &stamp));
        fs::write(venv.join(ENVIRONMENT_STAMP), &stamp).unwrap();
        assert!(environment_is_current(&venv, &stamp));
        assert!(!environment_is_current(&venv, "different\n"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn requirements_digest_changes_with_content() {
        let digest = content_digest("torch>=2.5");
        assert_eq!(digest.len(), 16);
        assert_eq!(digest, content_digest("torch>=2.5"));
        assert_ne!(digest, content_digest("torch>=2.6"));
        assert_ne!(digest, content_digest(""));
    }

    #[test]
    fn python_identity_reports_the_interpreter_and_its_version() {
        let python = PythonRuntime::Host(find_python().unwrap());
        let identity = python_identity(&python).unwrap();
        assert!(
            identity.contains("python"),
            "unexpected identity: {identity}"
        );
        assert!(identity.contains('.'), "no version in {identity}");
        assert!(python_identity(&PythonRuntime::Host(PathBuf::from("missing-python"))).is_err());
    }

    fn temporary_directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kassist-ezcompile-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
