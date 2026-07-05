use kome_runtime::{
    NativeRegistry,
    RuntimeError,
    Value,
};

pub fn native_registry() -> NativeRegistry {
    let mut registry = NativeRegistry::new();

    registry.register(
        "core.write_line",
        write_line,
    );

    registry
}

fn write_line(
    arguments: &[Value],
) -> Result<Value, RuntimeError> {
    let [value] = arguments else {
        return Err(RuntimeError::native(
            "core.write_line expects exactly one argument",
        ));
    };

    println!("{value}");

    Ok(Value::Null)
}