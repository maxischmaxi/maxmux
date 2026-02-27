//! Lua plugin system for maxmux.
//!
//! This crate provides:
//! - A [`HookRegistry`] with fire-and-forget and waterfall dispatch patterns.
//! - A [`LuaRuntime`] wrapping mlua with the `maxmux` API table.
//! - A [`PluginLoader`] that discovers and loads `.lua` files from a directory.

pub mod hooks;
pub mod loader;
pub mod lua;

pub use hooks::HookRegistry;
pub use loader::{PluginLoadResult, PluginLoader};
pub use lua::LuaRuntime;
