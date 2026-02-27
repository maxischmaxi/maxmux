use mlua::prelude::*;
use std::path::Path;

/// A Lua runtime environment with the `maxmux` API table pre-loaded.
///
/// Provides methods to execute scripts, call functions, and register additional
/// Rust functions into the `maxmux` global table.
pub struct LuaRuntime {
    lua: Lua,
}

impl LuaRuntime {
    /// Create a new Lua runtime with the `maxmux` API table initialized.
    pub fn new() -> Result<Self, LuaError> {
        let lua = Lua::new();
        Self::setup_api(&lua)?;
        Ok(Self { lua })
    }

    /// Set up the `maxmux` global table with built-in API functions.
    fn setup_api(lua: &Lua) -> Result<(), LuaError> {
        let maxmux = lua.create_table()?;

        // maxmux.log(msg) - log a message via tracing
        maxmux.set(
            "log",
            lua.create_function(|_, msg: String| {
                tracing::info!("[lua] {}", msg);
                Ok(())
            })?,
        )?;

        // maxmux.version() - return the current version string
        maxmux.set(
            "version",
            lua.create_function(|_, ()| Ok("0.1.0".to_string()))?,
        )?;

        lua.globals().set("maxmux", maxmux)?;
        Ok(())
    }

    /// Execute a Lua script string.
    pub fn exec(&self, script: &str) -> Result<(), LuaError> {
        self.lua.load(script).exec()
    }

    /// Execute a Lua file at the given path.
    pub fn exec_file(&self, path: &Path) -> Result<(), LuaError> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| LuaError::ExternalError(std::sync::Arc::new(e)))?;
        self.exec(&script)
    }

    /// Call a named global Lua function with the given arguments.
    pub fn call_function<A: IntoLuaMulti>(&self, name: &str, args: A) -> Result<(), LuaError> {
        let func: LuaFunction = self.lua.globals().get(name)?;
        func.call::<()>(args)?;
        Ok(())
    }

    /// Register a Rust function into the `maxmux` API table, callable from Lua.
    pub fn register_function<F>(&self, name: &str, func: F) -> Result<(), LuaError>
    where
        F: Fn(&Lua, LuaMultiValue) -> LuaResult<LuaMultiValue> + Send + 'static,
    {
        let maxmux: LuaTable = self.lua.globals().get("maxmux")?;
        maxmux.set(name, self.lua.create_function(func)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_simple_lua_script() {
        let runtime = LuaRuntime::new().expect("failed to create Lua runtime");
        // Set a global, then read it back to verify execution happened.
        runtime.exec("x = 40 + 2").expect("exec failed");

        let result: i64 = runtime
            .lua
            .globals()
            .get("x")
            .expect("failed to get global x");
        assert_eq!(result, 42);
    }

    #[test]
    fn maxmux_api_is_accessible_from_lua() {
        let runtime = LuaRuntime::new().expect("failed to create Lua runtime");

        // Verify maxmux.version() returns the expected string.
        runtime
            .exec("result = maxmux.version()")
            .expect("exec failed");

        let version: String = runtime
            .lua
            .globals()
            .get("result")
            .expect("failed to get result");
        assert_eq!(version, "0.1.0");
    }

    #[test]
    fn register_custom_function() {
        let runtime = LuaRuntime::new().expect("failed to create Lua runtime");

        runtime
            .register_function("add", |_, args: LuaMultiValue| {
                let mut iter = args.into_iter();
                let a: i64 = iter
                    .next()
                    .unwrap()
                    .as_integer()
                    .expect("expected integer");
                let b: i64 = iter
                    .next()
                    .unwrap()
                    .as_integer()
                    .expect("expected integer");
                Ok(LuaMultiValue::from_vec(vec![LuaValue::Integer(a + b)]))
            })
            .expect("register_function failed");

        runtime
            .exec("sum = maxmux.add(3, 7)")
            .expect("exec failed");

        let sum: i64 = runtime
            .lua
            .globals()
            .get("sum")
            .expect("failed to get sum");
        assert_eq!(sum, 10);
    }

    #[test]
    fn call_lua_function_from_rust() {
        let runtime = LuaRuntime::new().expect("failed to create Lua runtime");

        runtime
            .exec(
                r#"
                function greet(name)
                    greeting = "hello " .. name
                end
            "#,
            )
            .expect("exec failed");

        runtime
            .call_function("greet", "world")
            .expect("call_function failed");

        let greeting: String = runtime
            .lua
            .globals()
            .get("greeting")
            .expect("failed to get greeting");
        assert_eq!(greeting, "hello world");
    }
}
