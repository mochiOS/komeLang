use kome_ast::declarations::{Declaration, Module, UseImport};
use serde::Deserialize;
use std::{
    collections::{HashSet, VecDeque},
    env, fs,
    path::{Path, PathBuf},
};

pub const STDLIB_PATH_ENV: &str = "KOME_STDLIB_PATH";

const KOMEUP_HOME_ENV: &str = "KOMEUP_HOME";

pub struct StandardLibrary {
    root: PathBuf,
    prelude_path: PathBuf,
    prelude_source: String,
    prelude: Module,
}

#[derive(Debug, Deserialize)]
struct KomeupConfig {
    default_toolchain: String,
}

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub path: PathBuf,
    pub source: String,
    pub module: Module,
}

impl StandardLibrary {
    pub fn discover() -> Result<Self, String> {
        if let Some(raw_path) = env::var_os(STDLIB_PATH_ENV) {
            if raw_path.is_empty() {
                return Err(format!(
                    "{STDLIB_PATH_ENV} is set, \
                     but its value is empty",
                ));
            }

            return Self::load(PathBuf::from(raw_path));
        }

        let home = kome_home()?;
        let config_path = home.join("komeup.toml");

        let source = read_source(&config_path)?;

        let config = toml::from_str::<KomeupConfig>(&source)
            .map_err(|error| format!("failed to parse `{}`: {error}", config_path.display(),))?;

        let root = home
            .join("toolchains")
            .join(config.default_toolchain)
            .join("lib")
            .join("std");

        Self::load(root)
    }

    pub fn load_from_env() -> Result<Self, String> {
        let raw_path = env::var_os(STDLIB_PATH_ENV).ok_or_else(|| {
            format!(
                "{STDLIB_PATH_ENV} is not set; \
                         set it to the kome_std directory",
            )
        })?;

        if raw_path.is_empty() {
            return Err(format!(
                "{STDLIB_PATH_ENV} is set, \
                 but its value is empty",
            ));
        }

        Self::load(PathBuf::from(raw_path))
    }

    pub fn load(root: PathBuf) -> Result<Self, String> {
        let metadata = fs::metadata(&root).map_err(|error| {
            format!(
                "failed to access standard \
                         library directory `{}`: {error}",
                root.display(),
            )
        })?;

        if !metadata.is_dir() {
            return Err(format!(
                "standard library path `{}` \
                 is not a directory",
                root.display(),
            ));
        }

        let prelude_path = root.join("prelude.kome");

        let prelude_source = read_source(&prelude_path)?;

        let prelude = kome_parser::parse(&prelude_source)
            .map_err(|error| format!("{}: {error}", prelude_path.display(),))?;

        Ok(Self {
            root,
            prelude_path,
            prelude_source,
            prelude,
        })
    }

    pub fn modules_for(&self, application: &Module) -> Result<Vec<LoadedModule>, String> {
        let mut modules = vec![LoadedModule {
            path: self.prelude_path.clone(),
            source: self.prelude_source.clone(),
            module: self.prelude.clone(),
        }];

        let mut pending = VecDeque::new();

        pending.extend(standard_library_imports(&self.prelude));

        pending.extend(standard_library_imports(application));

        let mut loaded = HashSet::new();

        while let Some(path) = pending.pop_front() {
            let key = path.join(".");

            if !loaded.insert(key) {
                continue;
            }

            let loaded_module = self.load_module(&path)?;

            pending.extend(standard_library_imports(&loaded_module.module));

            modules.push(loaded_module);
        }

        Ok(modules)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn prelude_path(&self) -> &Path {
        &self.prelude_path
    }

    pub fn prelude(&self) -> &Module {
        &self.prelude
    }

    /// 従来どおりpreludeだけを結合します。
    pub fn merge_with(&self, mut application: Module) -> Module {
        let mut declarations =
            Vec::with_capacity(self.prelude.declarations.len() + application.declarations.len());

        declarations.extend(self.prelude.declarations.iter().cloned());

        declarations.append(&mut application.declarations);

        Module::new(declarations, application.span)
    }

    /// preludeと、アプリが`use std.*`で
    /// importしたモジュールを結合します。
    pub fn merge_with_imports(&self, mut application: Module) -> Result<Module, String> {
        let modules = self.modules_for(&application)?;

        let mut declarations = Vec::new();

        for loaded in modules {
            declarations.extend(
                loaded
                    .module
                    .declarations
                    .into_iter()
                    .filter(|declaration| !matches!(declaration, Declaration::Use(_),)),
            );
        }

        declarations.append(&mut application.declarations);

        Ok(Module::new(declarations, application.span))
    }

    fn load_module(&self, segments: &[String]) -> Result<LoadedModule, String> {
        if segments.first().map(String::as_str) != Some("std") {
            return Err(format!(
                "standard library module path \
                 must start with `std`: `{}`",
                segments.join("."),
            ));
        }

        if segments.len() < 2 {
            return Err("`std` must be followed by \
                 a module name"
                .to_owned());
        }

        let mut base = self.root.clone();

        for segment in &segments[1..] {
            base.push(segment);
        }

        let file_path = base.with_extension("kome");

        let module_path = base.join("mod.kome");

        let path = if file_path.is_file() {
            file_path
        } else if module_path.is_file() {
            module_path
        } else {
            return Err(format!(
                "standard library module `{}` \
                     was not found; expected `{}` \
                     or `{}`",
                segments.join("."),
                file_path.display(),
                module_path.display(),
            ));
        };

        let source = read_source(&path)?;

        let module =
            kome_parser::parse(&source).map_err(|error| format!("{}: {error}", path.display(),))?;

        Ok(LoadedModule {
            path,
            source,
            module,
        })
    }
}

fn standard_library_imports(module: &Module) -> Vec<Vec<String>> {
    let mut imports = Vec::new();

    for declaration in &module.declarations {
        let Declaration::Use(use_declaration) = declaration else {
            continue;
        };

        for import in &use_declaration.imports {
            let UseImport::Module(path) = import else {
                continue;
            };

            let segments = path
                .segments
                .iter()
                .map(|segment| segment.name.clone())
                .collect::<Vec<_>>();

            if segments.first().map(String::as_str) == Some("std") && segments.len() > 1 {
                imports.push(segments);
            }
        }
    }

    imports
}

fn kome_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(KOMEUP_HOME_ENV) {
        if path.is_empty() {
            return Err(format!(
                "{KOMEUP_HOME_ENV} is set, \
                 but its value is empty",
            ));
        }

        return Ok(PathBuf::from(path));
    }

    let home = env::var_os("HOME").ok_or_else(|| {
        "HOME is not set and \
                 KOMEUP_HOME was not provided"
            .to_owned()
    })?;

    Ok(PathBuf::from(home).join(".kome"))
}

fn read_source(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("failed to read `{}`: {error}", path.display(),))
}
