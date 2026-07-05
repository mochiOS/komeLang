use kome_ast::declarations::Module;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub const STDLIB_PATH_ENV: &str = "KOME_STDLIB_PATH";

pub struct StandardLibrary {
    prelude_path: PathBuf,
    prelude: Module,
}

impl StandardLibrary {
    pub fn load_from_env() -> Result<Self, String> {
        let raw_path = env::var_os(STDLIB_PATH_ENV).ok_or_else(|| {
            format!(
                "{STDLIB_PATH_ENV} is not set; set it to the kome_std directory"
            )
        })?;

        if raw_path.is_empty() {
            return Err(format!(
                "{STDLIB_PATH_ENV} is set, but its value is empty"
            ));
        }

        Self::load(PathBuf::from(raw_path))
    }

    pub fn load(root: PathBuf) -> Result<Self, String> {
        let metadata = fs::metadata(&root).map_err(|error| {
            format!(
                "failed to access standard library directory `{}`: {error}",
                root.display(),
            )
        })?;

        if !metadata.is_dir() {
            return Err(format!(
                "standard library path `{}` is not a directory",
                root.display(),
            ));
        }

        let prelude_path = root.join("prelude.kome");
        let source = read_source(&prelude_path)?;

        let prelude = kome_parser::parse(&source).map_err(|error| {
            format!("{}: {error}", prelude_path.display())
        })?;

        Ok(Self {
            prelude_path,
            prelude,
        })
    }

    pub fn prelude_path(&self) -> &Path {
        &self.prelude_path
    }

    pub fn prelude(&self) -> &Module {
        &self.prelude
    }

    pub fn merge_with(&self, mut application: Module) -> Module {
        let mut declarations = Vec::with_capacity(
            self.prelude.declarations.len()
                + application.declarations.len(),
        );

        declarations.extend(
            self.prelude.declarations.iter().cloned(),
        );

        declarations.append(&mut application.declarations);

        Module::new(declarations, application.span)
    }
}

fn read_source(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!("failed to read `{}`: {error}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kome_semantics::resolver::ScopeBuilder;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn prelude_declarations_are_visible_to_application() {
        let root = temporary_directory();

        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("prelude.kome"),
            r#"
@native("core.write_line")
fn write_line_native(value: String)

fn print(value: String) {
    write_line_native(value)
}
"#,
        )
            .unwrap();

        let standard_library =
            StandardLibrary::load(root.clone()).unwrap();

        let application = kome_parser::parse(
            r#"fn main() { print("Hello from Kome") }"#,
        )
            .unwrap();

        let module = standard_library.merge_with(application);
        let resolution = ScopeBuilder::resolve(&module);

        assert!(
            resolution.errors.is_empty(),
            "{:#?}",
            resolution.errors,
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn temporary_directory() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        env::temp_dir().join(format!(
            "komec-stdlib-test-{}-{timestamp}",
            std::process::id(),
        ))
    }
}