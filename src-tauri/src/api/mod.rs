//! API 命令模块（在 Android 上部分跳过）

#[cfg(not(target_os = "android"))]
pub mod adventure;
#[cfg(not(target_os = "android"))]
pub mod ambient;
#[cfg(not(target_os = "android"))]
pub mod asr;
#[cfg(not(target_os = "android"))]
pub mod character;
#[cfg(not(target_os = "android"))]
pub mod chat;
#[cfg(not(target_os = "android"))]
pub mod codex;
#[cfg(not(target_os = "android"))]
pub mod config;
#[cfg(not(target_os = "android"))]
pub mod font;
#[cfg(not(target_os = "android"))]
pub mod game;
#[cfg(not(target_os = "android"))]
pub mod live2d;
#[cfg(not(target_os = "android"))]
pub mod log;
#[cfg(not(target_os = "android"))]
pub mod music;
#[cfg(not(target_os = "android"))]
pub mod plugins;
#[cfg(not(target_os = "android"))]
pub mod role_archive;
#[cfg(not(target_os = "android"))]
pub mod save;
#[cfg(not(target_os = "android"))]
pub mod scene;
#[cfg(not(target_os = "android"))]
pub mod schedule;
#[cfg(not(target_os = "android"))]
pub mod script;
#[cfg(not(target_os = "android"))]
pub mod script_editor;
#[cfg(not(target_os = "android"))]
pub mod settings;
#[cfg(not(target_os = "android"))]
pub mod tool_settings;
#[cfg(not(target_os = "android"))]
pub mod voice;

// Android 上 api 模块用空桩
#[cfg(target_os = "android")]
pub mod api_stub {
    pub mod adventure {}
    pub mod ambient {}
    pub mod asr {}
    pub mod character {}
    pub mod chat {}
    pub mod codex {}
    pub mod config {}
    pub mod font {}
    pub mod game {}
    pub mod live2d {}
    pub mod log {}
    pub mod music {}
    pub mod plugins {}
    pub mod role_archive {}
    pub mod save {}
    pub mod scene {}
    pub mod schedule {}
    pub mod script {}
    pub mod script_editor {}
    pub mod settings {}
    pub mod tool_settings {}
    pub mod voice {}
    
    // 常用函数占位
    pub fn data_dir() -> std::path::PathBuf {
        std::path::PathBuf::from("/data")
    }
}

#[cfg(target_os = "android")]
pub use api_stub::*;
