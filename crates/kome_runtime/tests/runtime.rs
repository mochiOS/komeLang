use kome_runtime::{
    Interpreter,
    NativeRegistry,
    RuntimeError,
    Value,
};
use std::sync::{
    Arc,
    Mutex,
};

#[test]
fn executes_kome_wrapper_around_native_function() {
    let module = kome_parser::parse(
        r#"
@native("core.write_line")
fn write_line_native(value: String)

fn print(value: String) {
    write_line_native(value)
}

fn main() {
    print("Hello from Kome")
}
"#,
    )
        .unwrap();

    let output = Arc::new(Mutex::new(Vec::new()));
    let native_output = Arc::clone(&output);

    let mut natives = NativeRegistry::new();

    natives.register(
        "core.write_line",
        move |arguments| {
            let [Value::String(value)] = arguments else {
                return Err(RuntimeError::native(
                    "core.write_line expects one String",
                ));
            };

            native_output
                .lock()
                .unwrap()
                .push(value.clone());

            Ok(Value::Null)
        },
    );

    let mut interpreter =
        Interpreter::new(&module, &natives).unwrap();

    interpreter.run_entry("main").unwrap();

    assert_eq!(
        *output.lock().unwrap(),
        vec!["Hello from Kome".to_string()],
    );
}

#[test]
fn returns_error_when_entry_function_does_not_exist() {
    let module = kome_parser::parse(
        r#"
fn other() {
}
"#,
    )
        .unwrap();

    let natives = NativeRegistry::new();

    let mut interpreter =
        Interpreter::new(&module, &natives).unwrap();

    let error = interpreter
        .run_entry("main")
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeError::FunctionNotFound { name }
            if name == "main"
    ));
}

#[test]
fn passes_function_arguments_through_kome_functions() {
    let module = kome_parser::parse(
        r#"
@native("test.capture")
fn capture_native(value: String)

fn capture(value: String) {
    capture_native(value)
}

fn main() {
    capture("argument")
}
"#,
    )
        .unwrap();

    let output = Arc::new(Mutex::new(Vec::new()));
    let native_output = Arc::clone(&output);

    let mut natives = NativeRegistry::new();

    natives.register(
        "test.capture",
        move |arguments| {
            let [Value::String(value)] = arguments else {
                return Err(RuntimeError::native(
                    "test.capture expects one String",
                ));
            };

            native_output
                .lock()
                .unwrap()
                .push(value.clone());

            Ok(Value::Null)
        },
    );

    let mut interpreter =
        Interpreter::new(&module, &natives).unwrap();

    interpreter.run_entry("main").unwrap();

    assert_eq!(
        *output.lock().unwrap(),
        vec!["argument".to_string()],
    );
}