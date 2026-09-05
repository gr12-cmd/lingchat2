//! 初始化模块：DB、配置、AI 服务、聊天组件等。

pub mod role_sync;
pub mod static_copy;
pub mod voice_cleanup;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tauri::App;
use tauri::Emitter;
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

use crate::ChatComponents;
#[cfg(not(target_os = "android"))]
use crate::ai_service::emotion::EmotionClassifier;
use crate::ai_service::game_system::persistent_memory_system::MemorySectionLimits;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, migrate_if_needed, migrate_legacy_vision_keys,
    resolve_chat_provider, resolve_translate_provider,
};
use crate::ai_service::message_system::processor::{MessageProcessor, ProcessorOptions};
use crate::ai_service::service::{AIService, SharedAIService};
use crate::ai_service::translator::Translator;
#[cfg(not(target_os = "android"))]
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::CharacterSettings;
use crate::config::{self, AppConfig};
use crate::db;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;

// Android 上 emotion_classifier 的占位类型
#[cfg(target_os = "android")]
struct EmotionClassifier;

// Android 上 LocalTtsRuntime 的占位类型
#[cfg(target_os = "android")]
struct LocalTtsRuntime;

pub async fn initialize(
    app: &App,
    local_tts: Option<LocalTtsRuntime>,
) -> Result<(DatabaseConnection, SharedAIService, ChatComponents)> {
    // ... (前面的代码不变) ...

    let classifier = load_emotion_classifier(app_config.enable_emotion_classifier, &data_dir);
    let processor = Arc::new(MessageProcessor::new(
        ProcessorOptions {
            time_sense_enabled: app_config.enable_time_sense,
            enable_translate: app_config.enable_translate,
        },
        classifier,
    ));

    // ... (后面的代码不变) ...
}

/// ASR 服务初始化（仅桌面端）
#[cfg(not(target_os = "android"))]
pub async fn init_asr(
    app: &tauri::AppHandle,
    asr_state: &Arc<crate::ai_service::asr::AsrState>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ai_service::asr::{provider, session::AsrSession, settings, vad::AsrVad};

    tracing::info!("[ASR] init_asr 开始");
    let cfg = settings::load(app)?;
    let tls_config = crate::utils::tls::build_tls_config()?;
    let http = reqwest::Client::builder()
        .tls_backend_preconfigured(tls_config)
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut providers: std::collections::HashMap<
        String,
        std::sync::Arc<dyn provider::AsrProvider>,
    > = std::collections::HashMap::new();
    let cred = cfg
        .provider_configs
        .get(&cfg.active_provider)
        .map(|c| c.to_credentials())
        .unwrap_or_default();
    match provider::get_provider(&cfg.active_provider, &cred, &http).await {
        Ok(p) => {
            providers.insert(cfg.active_provider.clone(), p);
            tracing::info!("[ASR] provider {} 已构建", cfg.active_provider);
        },
        Err(e) => {
            tracing::warn!(
                "[ASR] provider {} 构建失败: {}",
                cfg.active_provider,
                e.i18n_code()
            );
        },
    }

    let vad = AsrVad::load(app)?;
    vad.set_silence_timeout_ms(cfg.vad_silence_ms).await;
    let session = Arc::new(AsrSession::new(Arc::new(vad), providers));
    *asr_state.session.lock().await = Some(session);

    let _ = app.emit("asr://vad_ready", ());

    tracing::info!("[ASR] init_asr 完成");
    Ok(())
}

/// Android 上 init_asr 为空实现
#[cfg(target_os = "android")]
pub async fn init_asr(
    _app: &tauri::AppHandle,
    _asr_state: &Arc<crate::ai_service::asr::AsrState>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("[ASR] init_asr 在 Android 上跳过");
    Ok(())
}

/// 加载情绪分类器（仅桌面端）
#[cfg(not(target_os = "android"))]
fn load_emotion_classifier(
    enabled: bool,
    data_dir: &std::path::Path,
) -> Option<Arc<EmotionClassifier>> {
    if !enabled {
        tracing::info!("情绪分类器已在配置中禁用");
        return None;
    }

    let model_dir = resolve_emotion_model_dir(data_dir);
    match model_dir {
        Some(dir) if dir.join("model.onnx").exists() => match EmotionClassifier::load(&dir) {
            Ok(clf) => {
                tracing::info!("情绪分类器加载成功: {}", dir.display());
                return Some(Arc::new(clf));
            },
            Err(e) => {
                tracing::warn!(
                    "情绪分类器加载失败 ({}), 回退为禁用状态: {e}",
                    dir.display()
                );
            },
        },
        _ => {
            tracing::warn!("未找到情绪模型目录, 情绪分类器将禁用");
        },
    }

    None
}

#[cfg(target_os = "android")]
fn load_emotion_classifier(
    _enabled: bool,
    _data_dir: &std::path::Path,
) -> Option<Arc<EmotionClassifier>> {
    tracing::info!("情绪分类器在 Android 上跳过");
    None
}

#[cfg(not(target_os = "android"))]
fn resolve_emotion_model_dir(data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let data_path = data_dir.join("third_party").join("emotion_model_19emo");
    if data_path.exists() {
        return Some(data_path);
    }
    None
}

#[cfg(target_os = "android")]
fn resolve_emotion_model_dir(_data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    None
}

/// 加载默认角色设定：上次游玩的角色 → 第一个主角色 → 默认空设定
async fn load_default_character(
    app: &App,
    db: &DatabaseConnection,
    data_dir: &std::path::Path,
) -> Result<CharacterSettings> {
    // ... (保持不变) ...
}
