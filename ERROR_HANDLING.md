# Error Handling Architecture

## Overview

This project uses a unified error handling strategy across Rust and C++ components, leveraging the `anyhow` crate on the Rust side and a custom `Result<T>` type on the C++ side.

## Rust Side Error Handling

### anyhow::Error

The Rust codebase uses `anyhow::Error` as the primary error type, providing:
- Rich error context with `.context()`
- Easy error propagation with `?` operator
- Flexible error types from any source

### Example Usage

```rust
use anyhow::{Context, Result};

fn timeline_of(project: &Project, index: usize) -> Result<Timeline> {
    let timeline = project.timeline(index)
        .context("Failed to get timeline")?;
    Ok(timeline)
}
```

### FFI Boundary Conversion

At the FFI boundary, `anyhow::Error` is converted to `WrapperErrorCode`:

```rust
impl WrapperErrorCode {
    pub fn from_anyhow(err: anyhow::Error) -> Self {
        let error_msg = err.to_string();
        
        if error_msg.contains("not found") || error_msg.contains("NotFound") {
            Self::not_found(Some(&error_msg))
        } else if error_msg.contains("null") || error_msg.contains("NullPtr") {
            Self::null_ptr()
        } else if error_msg.contains("panic") || error_msg.contains("Panic") {
            Self::panic(Some(&error_msg))
        } else {
            Self::error(Some(&error_msg))
        }
    }
}
```

## C++ Side Error Handling

### Result<T> Type

The C++ codebase provides a `Result<T>` template type that mimics Rust's Result:

```cpp
template<typename T>
class Result {
private:
    std::variant<T, std::string> value;
    
public:
    static Result ok(T val);
    static Result error(std::string msg);
    bool isOk() const;
    bool isError() const;
    std::optional<T> okValue() const;
    std::optional<std::string> errorMessage() const;
    T& unwrap();
    T unwrapOr(T default_val) const;
    
    template<typename F>
    auto map(F&& f) const -> Result<decltype(f(std::declval<T>()))>;
    
    template<typename F>
    auto andThen(F&& f) const -> decltype(f(std::declval<T>()));
};
```

### Example Usage

```cpp
auto projectResult = network->getProject();
if (projectResult.isError()) {
    // Handle error
    return;
}
Project project = projectResult.unwrap();
```

### FFI Boundary Conversion

The C++ side converts `WrapperErrorCode` to `Result<T>`:

```cpp
template<typename T>
Result<T> wrapperResultToResult(WrapperErrorCode code, T value = T{}) {
    const char *msg = esotereel_gui_helper::get_last_err_msg();
    
    switch (code) {
    case WrapperErrorCode::Ok:
        return Result<T>::ok(std::move(value));
    case WrapperErrorCode::NotFound:
        return Result<T>::error(std::string("Not found: ") + (msg ? msg : ""));
    case WrapperErrorCode::Error:
        return Result<T>::error(std::string("Error: ") + (msg ? msg : ""));
    case WrapperErrorCode::NullPtr:
        return Result<T>::error(std::string("Null pointer: ") + (msg ? msg : ""));
    case WrapperErrorCode::Panic:
        return Result<T>::error(std::string("Panic: ") + (msg ? msg : ""));
    default:
        return Result<T>::error(std::string("Unknown error"));
    }
}
```

## Error Flow

1. **Rust Internal**: Functions return `Result<T>` (alias for `anyhow::Result<T>`)
2. **FFI Boundary**: `anyhow::Error` → `WrapperErrorCode` with error message
3. **C++ Boundary**: `WrapperErrorCode` → `Result<T>` with error message
4. **C++ Internal**: Functions use `Result<T>` for error handling

## Migration from Old System

### Before
- Multiple error systems: C++ exceptions, error codes, invalid objects
- Inconsistent error handling across the codebase
- No unified error context

### After
- Single error type per language (`anyhow::Error` in Rust, `Result<T>` in C++)
- Consistent error propagation
- Rich error context throughout the stack
- Type-safe error handling

## Best Practices

### Rust
- Use `.context()` to add error context
- Avoid `unwrap()` in production code - use `?` instead
- Keep error messages descriptive
- Use specific error types when needed

### C++
- Check `isError()` before accessing values
- Use `unwrapOr()` for sensible defaults
- Prefer `map()` and `andThen()` for chaining operations
- Avoid throwing exceptions - use `Result<T>` instead

## Future Improvements

1. Add more specific error types in Rust for better error classification
2. Implement error recovery strategies at the application level
3. Add error logging and monitoring
4. Consider adding error tracing for debugging

## Related Files

- `lib/src/util/result.rs` - Rust error handling utilities
- `guihlp/src/lib.rs` - FFI error conversion
- `gui/src/wrapper/result.h` - C++ Result<T> implementation
- `gui/src/wrapper/exception.h` - Legacy exception handling (deprecated)