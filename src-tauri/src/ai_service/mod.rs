//! AI 服务模块（在 Android 上完全跳过）

#[cfg(not(target_os = "android"))]
pub mod asr;
#[cfg(not(target_os = "android"))]
pub mod emotion;
#[cfg(not(target_os = "android"))]
pub mod game_system;
#[cfg(not(target_os = "android"))]
pub mod god_agent;
#[cfg(not(target_os = "android"))]
pub mod llm;
#[cfg(not(target_os = "android"))]
pub mod message_system;
#[cfg(not(target_os = "android"))]
pub mod proactive_system;
#[cfg(not(target_os = "android"))]
pub mod screen_analyzer;
#[cfg(not(target_os = "android"))]
pub mod service;
#[cfg(not(target_os = "android"))]
pub mod skill_agent;
#[cfg(not(target_os = "android"))]
pub mod tools;
#[cfg(not(target_os = "android"))]
pub mod translator;
#[cfg(not(target_os = "android"))]
pub mod tts;
#[cfg(not(target_os = "android"))]
pub mod types;

// Android 上整个 ai_service 是空的
#[cfg(target_os = "android")]
pub mod ai_service_stub {
    // 所有类型都定义为空
    pub mod asr {}
    pub mod emotion {}
    pub mod game_system {
        pub mod persistent_memory_system {
            pub struct MemorySectionLimits;
        }
        pub mod game_status {
            pub struct GameStatus;
            pub struct GameStatusSnapshot;
        }
        pub mod script_engine {
            pub struct ScriptManager;
            pub struct ScriptChannels;
            pub mod events {
                pub struct ScriptContext;
                pub mod background_effect_event {
                    pub const KNOWN_EFFECTS: &[&str] = &[];
                }
            }
            pub mod utils {
                pub mod media {
                    pub enum MediaType {}
                }
                pub mod script_function {}
            }
        }
        pub mod auto_save {
            pub struct AutoSaveManager;
        }
        pub mod scene_store {
            pub struct Scene;
            pub struct SceneStore;
            pub struct LightingParams;
        }
    }
    pub mod god_agent {
        pub mod config {
            pub struct GodAgentConfig;
            pub fn resolve_god_agent_provider() {}
        }
        pub struct GodAgentCore;
    }
    pub mod llm {
        pub struct LlmSlot;
        pub mod provider_config {
            pub fn migrate_if_needed() {}
            pub fn migrate_legacy_vision_keys() {}
            pub fn resolve_chat_provider() {}
            pub fn resolve_translate_provider() {}
            pub fn build_llm_client_from_provider() {}
        }
        pub mod error {
            pub struct LlmErrorPayload;
            pub fn classify_llm_error() {}
        }
        pub mod factory {}
        pub mod codex_auth {}
        pub async fn slot_snapshot() -> Option<()> { None }
    }
    pub mod message_system {
        pub mod processor {
            pub struct MessageProcessor;
            pub struct ProcessorOptions;
            pub struct EmotionSegment;
        }
        pub mod generator {
            pub struct GeneratorSource;
        }
        pub mod events {}
    }
    pub mod proactive_system {
        pub mod types {
            pub struct UserScheduleSettings;
        }
        pub struct ProactiveSystem;
    }
    pub mod screen_analyzer {
        pub struct ScreenAnalyzer;
        pub struct ScreenAnalyzerConfig;
    }
    pub mod service {
        pub struct AIService;
        pub type SharedAIService = std::sync::Arc<tokio::sync::Mutex<AIService>>;
    }
    pub mod skill_agent {
        pub struct SkillAgentState;
        pub mod config {
            pub struct SkillAgentConfig;
            pub fn resolve_skill_agent_provider() {}
        }
        pub mod core {
            pub struct SkillAgentRunContext;
            pub fn run_chat() {}
        }
        pub mod events {
            pub struct SkillAgentEvent;
        }
        pub mod db {}
        pub mod skills {}
        pub mod command_executor {}
        pub mod ApprovalMap;
        pub async fn ensure_skills_dir() -> Result<(), anyhow::Error> { Ok(()) }
    }
    pub mod tools {
        pub mod registry {
            pub struct ToolRegistry;
        }
        pub mod executor {
            pub struct Tool;
            pub struct ToolContext;
            pub struct ToolError;
            pub type ToolResult<T> = Result<T, ToolError>;
        }
        pub mod settings {
            pub struct ToolSettings;
            pub struct SharedToolSettings;
        }
        pub mod web_search {
            pub struct WebSearchTool;
        }
        pub mod permissions {
            pub const CONFIG_FILE_NAME: &str = "";
        }
        pub mod background_command {
            pub struct BackgroundCommandManager;
        }
        pub mod built_in_registry {}
    }
    pub mod translator {
        pub struct Translator;
    }
    pub mod tts {
        pub mod local {
            pub struct LocalTtsRuntime;
            pub struct LocalTtsState;
            pub mod setup {
                pub fn bootstrap() -> Result<LocalTtsRuntime, anyhow::Error> {
                    Ok(LocalTtsRuntime)
                }
                pub fn spawn_preload() {}
            }
            pub mod saf_bridge {
                pub struct ImportSource;
                pub async fn prepare_file_import_source() -> Result<ImportSource, anyhow::Error> {
                    Ok(ImportSource)
                }
            }
            pub mod commands {
                pub fn tts_local_status() {}
                pub fn tts_local_list_catalog() {}
                pub fn tts_local_list_installed() {}
                pub fn tts_local_import_from_path() {}
                pub fn tts_local_download() {}
                pub fn tts_local_delete_voice() {}
                pub fn tts_local_delete_deberta() {}
                pub fn tts_local_import_style_vectors() {}
                pub fn tts_local_synthesize_preview() {}
                pub fn tts_local_get_enabled() {}
                pub fn tts_local_set_enabled() {}
                pub fn tts_local_get_device() {}
                pub fn tts_local_list_devices() {}
                pub fn tts_local_set_device() {}
            }
        }
        pub mod cloud {
            pub mod commands {
                pub fn cosyvoice_get_config() {}
                pub fn cosyvoice_save_api_key() {}
                pub fn cosyvoice_create_voice() {}
                pub fn cosyvoice_voice_status() {}
                pub fn cosyvoice_list_voices() {}
                pub fn cosyvoice_delete_voice() {}
                pub fn cosyvoice_synthesize_preview() {}
            }
        }
    }
    pub mod types {
        pub struct CharacterSettings;
        pub struct ScriptStatus;
        pub struct LineBase;
        pub struct LineAttributeExt;
        pub struct Live2dSettings;
        pub struct LlmMessage;
        pub struct ToolDefinition;
        pub struct GameLine;
    }
}

#[cfg(target_os = "android")]
pub use ai_service_stub::*;
