use kome_runtime::{NativeRegistry, RuntimeError, Value};
use std::io;
use std::io::Write;

pub fn native_registry() -> NativeRegistry {
    let mut registry = NativeRegistry::new();

    registry.register("core.write", write);

    registry
}

fn write(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let [value] = arguments else {
        return Err(RuntimeError::native(
            "core.write expects exactly one argument",
        ));
    };

    print!("{value}");

    io::stdout()
        .flush()
        .map_err(|error| RuntimeError::native(format!("failed to flush stdout: {error}")))?;

    Ok(Value::Null)
}
