#![cfg_attr(target_os = "android", allow(unused_imports, dead_code, unused_variables))]

mod achievements;
mod adventures;
mod ai_service;
mod api;
mod cast;
mod config;
mod db;
mod init;
mod lan_sync;
mod manifest;
mod migration;
mod plugins;
mod resource_sync;
pub mod utils;

use std::sync::Arc;

use chrono::Local;
use sea_orm::DatabaseConnection;
#[cfg(desktop)]
use tauri::Emitter;
use tauri::{Listener, Manager};
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[cfg(not(target_os = "android"))]
use ai_service::god_agent::GodAgentCore;
#[cfg(not(target_os = "android"))]
use ai_service::god_agent::config::resolve_god_agent_provider;
#[cfg(not(target_os = "android"))]
use ai_service::llm::LlmSlot;
#[cfg(not(target_os = "android"))]
use ai_service::message_system::processor::MessageProcessor;
#[cfg(not(target_os = "android"))]
use ai_service::screen_analyzer::{ScreenAnalyzer, ScreenAnalyzerConfig};
#[cfg(not(target_os = "android"))]
use ai_service::service::SharedAIService;
#[cfg(not(target_os = "android"))]
use ai_service::tools::registry::ToolRegistry;
#[cfg(not(target_os = "android"))]
use ai_service::translator::Translator;

#[cfg(target_os = "android")]
type LlmSlot = ();
#[cfg(target_os = "android")]
type SharedAIService = ();
#[cfg(target_os = "android")]
type ToolRegistry = ();

/// 本地时间格式化器，用于日志输出的时间戳。
struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%H:%M:%S"))
    }
}

fn build_log_filter(genai_debug: bool) -> tracing_subscriber::EnvFilter {
    let base = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,ling_chat_lib=info"))
        .add_directive("sqlx=warn".parse().unwrap());
    if genai_debug {
        base.add_directive("genai=debug".parse().unwrap())
    } else {
        base.add_directive("genai=error".parse().unwrap())
    }
}

#[cfg(not(target_os = "android"))]
pub struct ChatComponents {
    pub llm: LlmSlot,
    pub processor: Arc<MessageProcessor>,
    pub translator: Arc<Translator>,
}

#[cfg(target_os = "android")]
pub struct ChatComponents;

#[derive(Default)]
pub struct ScreenshotCaptureState {
    pub full_capture_base64: Option<String>,
    pub overlay_label: Option<String>,
}

#[cfg(not(target_os = "android"))]
pub struct InnerAppState {
    pub db: DatabaseConnection,
    pub ai_service: SharedAIService,
    pub chat: ChatComponents,
    pub script_channels: ai_service::game_system::script_engine::SharedScriptChannels,
    pub generation_lock: Arc<tokio::sync::Mutex<()>>,
    pub tool_registry: Arc<ToolRegistry>,
    pub tool_settings: ai_service::tools::settings::SharedToolSettings,
    pub plugin_manager: Arc<plugins::PluginManager>,
    pub proactive_system: Option<Arc<tokio::sync::Mutex<ai_service::proactive_system::ProactiveSystem>>>,
    pub achievement_manager: Arc<tokio::sync::Mutex<achievements::manager::AchievementManager>>,
    pub screen_analyzer: Arc<tokio::sync::Mutex<ScreenAnalyzer>>,
    pub screenshot_capture: Arc<tokio::sync::Mutex<ScreenshotCaptureState>>,
    pub auto_save_manager: Arc<tokio::sync::Mutex<ai_service::game_system::auto_save::AutoSaveManager>>,
    pub asr_state: Arc<ai_service::asr::AsrState>,
    pub god_agent: Option<Arc<GodAgentCore>>,
    pub skill_agent: Arc<ai_service::skill_agent::SkillAgentState>,
    pub chat_command_approvals: ai_service::skill_agent::ApprovalMap,
    pub chat_file_change_approvals: ai_service::skill_agent::ApprovalMap,
    pub chat_file_delete_approvals: ai_service::skill_agent::ApprovalMap,
    pub background_commands: Arc<ai_service::tools::background_command::BackgroundCommandManager>,
    pub preview_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub pending_preview_restore: Arc<tokio::sync::Mutex<Option<api::script_editor::PreviewSession>>>,
}

#[cfg(target_os = "android")]
pub struct InnerAppState {
    pub db: DatabaseConnection,
    pub screenshot_capture: Arc<tokio::sync::Mutex<ScreenshotCaptureState>>,
    pub preview_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub pending_preview_restore: Arc<tokio::sync::Mutex<Option<api::script_editor::PreviewSession>>>,
}

pub struct AppState {
    inner: std::sync::OnceLock<InnerAppState>,
}

impl AppState {
    pub fn empty() -> Self {
        Self {
            inner: std::sync::OnceLock::new(),
        }
    }

    pub fn fill(&self, inner: InnerAppState) {
        if self.inner.set(inner).is_err() {
            panic!("AppState already filled");
        }
    }

    pub fn data(&self) -> &InnerAppState {
        self.inner
            .get()
            .expect("AppState accessed before initialization")
    }
}

#[cfg(not(target_os = "android"))]
impl std::ops::Deref for AppState {
    type Target = InnerAppState;
    fn deref(&self) -> &Self::Target {
        self.inner
            .get()
            .expect("AppState accessed before initialization")
    }
}

#[cfg(target_os = "android")]
impl std::ops::Deref for AppState {
    type Target = InnerAppState;
    fn deref(&self) -> &Self::Target {
        loop {
            if let Some(inner) = self.inner.get() {
                return inner;
            }
            std::hint::spin_loop();
        }
    }
}

#[cfg(target_os = "windows")]
fn read_hdr_mode_enabled(identifier: &str) -> bool {
    use serde_json::Value;
    let Some(appdata) = std::env::var("APPDATA").ok() else {
        return false;
    };
    let path = std::path::Path::new(&appdata)
        .join(identifier)
        .join(crate::config::STORE_FILE);
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    json.get(crate::config::keys::HDR_MODE_ENABLED)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let (filter, reload_handle) = tracing_subscriber::reload::Layer::new(build_log_filter(false));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_timer(LocalTimer))
        .with(utils::log_bridge::LogBridgeLayer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(utils::file_logger::LogFileWriter)
                .with_timer(LocalTimer)
                .with_ansi(false),
        )
        .with(filter)
        .init();

    let context = tauri::generate_context!();

    #[cfg(target_os = "windows")]
    {
        if !read_hdr_mode_enabled(&context.config().identifier) {
            #[allow(deprecated)]
            unsafe {
                std::env::set_var(
                    "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                    "--force-color-profile=scrgb-linear",
                );
            }
        }
    }

    // =====================================================
    // Android 简化版：只保留基础功能
    // =====================================================
    #[cfg(target_os = "android")]
    {
        tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_store::Builder::new().build())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_screenshots::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_android_fs::init())
            .setup(|app| {
                init::static_copy::init_data_dir(&app.handle());
                app.manage(AppState::empty());
                let rt = tokio::runtime::Runtime::new()?;
                let (db, _, _) = rt.block_on(init::initialize(app, None))?;

                let state = app.state::<AppState>();
                state.fill(InnerAppState {
                    db,
                    screenshot_capture: Arc::new(tokio::sync::Mutex::new(
                        ScreenshotCaptureState::default(),
                    )),
                    preview_task: Arc::new(tokio::sync::Mutex::new(None)),
                    pending_preview_restore: Arc::new(tokio::sync::Mutex::new(None)),
                });

                // 日志配置
                {
                    let store = config::settings_store(app.handle()).ok();
                    let log_enable = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::LOG_ENABLE))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let retention_days = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::LOG_RETENTION_DAYS))
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32)
                        .unwrap_or(10);

                    let data_dir = init::static_copy::get_data_dir();
                    utils::file_logger::init_logging(data_dir, log_enable);
                    utils::file_logger::cleanup_old_logs(retention_days);
                }

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                utils::log_bridge::get_log_history,
                utils::log_bridge::open_log_window,
                utils::log_bridge::is_log_window_open,
                api::settings::get_settings_tree,
                api::settings::save_settings,
                api::settings::get_setting_by_key,
                api::settings::select_file,
                api::settings::list_llm_providers,
                api::settings::save_llm_provider,
                api::settings::delete_llm_provider,
                api::settings::list_llm_models,
                api::font::list_system_fonts,
                api::font::import_font,
                api::font::list_imported_fonts,
                api::font::delete_imported_font,
                api::character::get_character_list,
                api::character::get_role_info,
                api::character::get_role_settings,
                api::character::get_character_file,
                api::character::get_avatar_file,
                api::character::update_role_settings,
                api::character::delete_character,
                api::character::open_characters_folder,
                api::live2d::import_live2d,
                api::live2d::get_live2d_file,
                api::live2d::inspect_live2d,
                api::background::get_background_list,
                api::background::get_background_file,
                api::background::upload_background_image,
                api::background::open_backgrounds_folder,
                api::music::get_music_list,
                api::music::get_music_file,
                api::music::upload_music,
                api::music::delete_music,
                api::music::save_bgm_state,
                api::asset::get_asset_base64,
                api::game::init_game,
                api::game::select_character,
                api::game::clear_conversation,
                api::game::update_voice_lang,
                api::chat::send_chat_message,
                api::chat::rollback_conversation,
                api::chat::generate_line_voice,
                api::chat::feed_image,
                api::chat::feed_text,
                api::save::list_saves,
                api::save::create_save,
                api::save::load_save,
                api::save::update_save,
                api::save::delete_save,
                api::save::update_save_title,
                api::script::list_scripts,
                api::script::list_standalone_scripts,
                api::script::start_script,
                api::script::script_submit_input,
                api::script::script_submit_choice,
                api::script_editor::editor_get_schema,
                api::script_editor::editor_list_scripts,
                api::script_editor::editor_read_script,
                api::script_editor::editor_read_chapter,
                api::script_editor::editor_validate_script,
                api::script_editor::editor_write_chapter,
                api::script_editor::editor_write_story_config,
                api::script_editor::editor_create_chapter,
                api::script_editor::editor_delete_chapter,
                api::script_editor::editor_delete_character,
                api::script_editor::editor_create_script,
                api::script_editor::editor_delete_script,
                api::script_editor::editor_upload_asset,
                api::script_editor::editor_create_character,
                api::script_editor::editor_list_global_assets,
                api::script_editor::editor_list_asset_files,
                api::script_editor::editor_delete_asset,
                api::script_editor::editor_rescan_scripts,
                api::script_editor::editor_start_preview,
                api::script_editor::editor_stop_preview,
                api::script_editor::editor_open_script_folder,
                api::pet::update_solid_regions,
                api::pet::set_pet_mode,
                api::schedule::get_schedules,
                api::schedule::save_schedules,
                api::adventure::list_character_adventures,
                api::adventure::list_all_adventures,
                api::adventure::start_adventure,
                api::adventure::check_adventure_unlocks,
                api::adventure::reset_adventure,
                api::role_archive::import_role,
                api::role_archive::import_role_from_path,
                api::role_archive::cancel_role_import,
                api::role_archive::rescan_roles,
                api::role_archive::export_role,
                api::role_archive::export_role_to_path,
                exit_app,
            ])
            .run(context)
            .expect("error while running tauri application");
        return;
    }

    // =====================================================
    // 桌面端完整流程
    // =====================================================
    #[cfg(not(target_os = "android"))]
    {
        let builder = tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_store::Builder::new().build())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_screenshots::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_android_fs::init());

        #[cfg(desktop)]
        let builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());

        builder
            .setup(|app| {
                utils::log_bridge::set_app_handle(app.handle().clone());
                init::static_copy::init_data_dir(&app.handle());

                app.manage(api::pet::HitTestState::default());
                app.manage(resource_sync::ResourceSyncState::default());
                app.manage(lan_sync::LanSyncState::default());
                app.manage(cast::CastManager::default());
                app.manage(utils::cpu_perf::CpuDetectionCache::new());
                app.manage(utils::gpu_perf::GpuDetectionCache::new());
                app.manage(api::role_archive::RoleArchiveState::default());

                app.manage(AppState::empty());
                let rt = tokio::runtime::Runtime::new()?;
                let local_tts = ai_service::tts::local::setup::bootstrap(app)?;
                let (db, ai_service, chat) =
                    rt.block_on(init::initialize(app, Some(local_tts.runtime.clone())))?;

                // 日志配置
                {
                    let store = config::settings_store(app.handle()).ok();
                    let log_enable = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::LOG_ENABLE))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let retention_days = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::LOG_RETENTION_DAYS))
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32)
                        .unwrap_or(10);

                    let data_dir = init::static_copy::get_data_dir();
                    utils::file_logger::init_logging(data_dir, log_enable);
                    utils::file_logger::cleanup_old_logs(retention_days);

                    let llm_request_log_enable = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::LOG_LLM_REQUEST_BODY))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    utils::llm_request_logger::init(data_dir, llm_request_log_enable);

                    let genai_debug = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::LOG_GENAI_DEBUG))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if let Err(e) = reload_handle.reload(build_log_filter(genai_debug)) {
                        tracing::warn!("应用日志过滤器失败: {e}");
                    }
                }

                let app_handle = app.handle().clone();
                app_handle.listen("store://change", move |event| {
                    #[derive(serde::Deserialize)]
                    struct StoreChangePayload {
                        key: String,
                        value: Option<serde_json::Value>,
                    }
                    let Ok(payload) = serde_json::from_str::<StoreChangePayload>(event.payload())
                    else {
                        return;
                    };
                    if payload.key != config::keys::LOG_GENAI_DEBUG {
                        return;
                    }
                    let genai_debug = matches!(payload.value, Some(serde_json::Value::Bool(true)));
                    if let Err(e) = reload_handle.reload(build_log_filter(genai_debug)) {
                        tracing::warn!("热重载 genai 调试日志失败: {e}");
                    }
                });

                match rt.block_on(init::voice_cleanup::cleanup_orphan_voice_files(
                    &db,
                    app.handle(),
                )) {
                    Ok(stats) => {
                        tracing::info!("语音文件清理完成: 删除 {} 个文件", stats.deleted_count);
                    }
                    Err(e) => {
                        tracing::warn!("语音文件清理失败（非致命错误）: {e:#}");
                    }
                }

                let script_channels = std::sync::Arc::new(tokio::sync::Mutex::new(
                    ai_service::game_system::script_engine::ScriptChannels::new(),
                ));

                let generation_lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
                let role_names = rt.block_on(
                    db::managers::role_repo::RoleRepo::get_all_tool_role_names(&db),
                )?;
                let tool_settings = ai_service::tools::settings::SharedToolSettings::new(
                    ai_service::tools::settings::ToolSettings::load_or_create(&api::data_dir())?,
                );
                let tool_registry = Arc::new(ai_service::tools::built_in_registry(
                    role_names,
                    tool_settings.clone(),
                )?);

                let plugin_manager = {
                    let data_dir = api::data_dir();
                    let plugins_root = data_dir.join("plugins");
                    if std::fs::create_dir_all(&plugins_root).is_err() {
                        tracing::warn!("插件目录创建失败: {}", plugins_root.display());
                    }
                    let manager = Arc::new(plugins::PluginManager::new(
                        data_dir.clone(),
                        tool_registry.clone(),
                    ));
                    if let Err(e) = tool_registry.save_permissions(&data_dir) {
                        tracing::warn!("插件注册后保存权限配置失败: {e}");
                    }
                    manager
                };

                let proactive = std::sync::Arc::new(tokio::sync::Mutex::new(
                    ai_service::proactive_system::ProactiveSystem::new(
                        app.handle().clone(),
                        db.clone(),
                        ai_service.clone(),
                        ChatComponents {
                            llm: chat.llm.clone(),
                            processor: chat.processor.clone(),
                            translator: chat.translator.clone(),
                        },
                        tool_registry.clone(),
                        generation_lock.clone(),
                    ),
                ));

                let proactive_clone = proactive.clone();
                tauri::async_runtime::spawn(async move {
                    ai_service::proactive_system::ProactiveSystem::start(proactive_clone).await;
                });

                let achievement_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
                    achievements::manager::AchievementManager::new(&api::data_dir()),
                ));

                let screen_analyzer = {
                    let sa_config = ScreenAnalyzerConfig::resolve(&app.handle());
                    std::sync::Arc::new(tokio::sync::Mutex::new(ScreenAnalyzer::new(sa_config)))
                };

                let screenshot_capture =
                    std::sync::Arc::new(tokio::sync::Mutex::new(ScreenshotCaptureState::default()));

                let auto_save_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
                    ai_service::game_system::auto_save::AutoSaveManager::new(
                        app.handle().clone(),
                        db.clone(),
                        ai_service.clone(),
                    ),
                ));

                let god_agent = resolve_god_agent_provider(&app.handle()).map(|llm| {
                    let config = ai_service::god_agent::config::GodAgentConfig::load(&app.handle());
                    let slot: LlmSlot =
                        std::sync::Arc::new(tokio::sync::RwLock::new(Some(Arc::new(llm))));
                    Arc::new(GodAgentCore::new(slot, config))
                });

                if let Err(e) = ai_service::skill_agent::ensure_skills_dir(&api::data_dir()) {
                    tracing::warn!("Skill Agent 技能库目录初始化失败: {}", e);
                }

                {
                    let state = app.state::<AppState>();
                    state.fill(InnerAppState {
                        db,
                        ai_service,
                        chat,
                        script_channels,
                        generation_lock,
                        tool_registry,
                        tool_settings,
                        plugin_manager,
                        proactive_system: Some(proactive),
                        achievement_manager,
                        screen_analyzer,
                        screenshot_capture,
                        auto_save_manager: auto_save_manager.clone(),
                        asr_state: Arc::new(ai_service::asr::AsrState {
                            session: Arc::new(tokio::sync::Mutex::new(None)),
                        }),
                        god_agent,
                        skill_agent: Arc::new(ai_service::skill_agent::SkillAgentState::default()),
                        chat_command_approvals: Default::default(),
                        chat_file_change_approvals: Default::default(),
                        chat_file_delete_approvals: Default::default(),
                        background_commands: Arc::new(
                            ai_service::tools::background_command::BackgroundCommandManager::default(),
                        ),
                        preview_task: Arc::new(tokio::sync::Mutex::new(None)),
                        pending_preview_restore: Arc::new(tokio::sync::Mutex::new(None)),
                    });
                }

                {
                    let state = app.state::<AppState>();
                    let asr_state = state.asr_state.clone();
                    if let Err(e) = rt.block_on(init::init_asr(app.handle(), &asr_state)) {
                        tracing::warn!("[ASR] init_asr 失败，ASR 功能不可用: {e:#}");
                    }
                }

                {
                    let store = config::settings_store(app.handle()).ok();
                    let cast_enabled = store
                        .as_ref()
                        .and_then(|s| s.get(config::keys::CAST_ENABLED))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if cast_enabled {
                        let app_handle = app.handle().clone();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                            let cast = app_handle.state::<cast::CastManager>();
                            if let Err(e) = cast::start_cast_server(&app_handle, &cast).await {
                                tracing::warn!("[Cast] 启动时自动开启投屏失败: {e}");
                            }
                        });
                    }
                }

                rt.block_on(api::plugins::refresh_plugin_content(app.handle()));

                ai_service::tts::local::setup::spawn_preload(&app.handle(), &local_tts);

                let window = app
                    .get_webview_window("main")
                    .ok_or_else(|| tauri::Error::AssetNotFound("main window not found".to_string()))?;

                ai_service::game_system::auto_save::AutoSaveManager::setup_close_handler(
                    app.handle().clone(),
                    window.clone(),
                    auto_save_manager.clone(),
                );

                tauri::async_runtime::spawn(async move {
                    ai_service::game_system::auto_save::AutoSaveManager::run_periodic(
                        auto_save_manager,
                    )
                    .await;
                });

                #[cfg(desktop)]
                {
                    let hit_test_state = app.state::<api::pet::HitTestState>();
                    let rects_arc = hit_test_state.solid_rects.clone();
                    let enabled_arc = hit_test_state.enabled.clone();

                    tauri::async_runtime::spawn(async move {
                        let mut was_ignored = false;
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                            let enabled = if let Ok(locked) = enabled_arc.lock() {
                                *locked
                            } else {
                                false
                            };

                            if !enabled {
                                if was_ignored {
                                    let _ = window.set_ignore_cursor_events(false);
                                    was_ignored = false;
                                }
                                continue;
                            }

                            let Ok(cursor) = window.cursor_position() else {
                                continue;
                            };

                            if let Ok(window_pos) = window.outer_position() {
                                if let Ok(scale_factor) = window.scale_factor() {
                                    let mouse_x = cursor.x - f64::from(window_pos.x);
                                    let mouse_y = cursor.y - f64::from(window_pos.y);

                                    let logical_x = mouse_x / scale_factor;
                                    let logical_y = mouse_y / scale_factor;

                                    let _ = window.emit(
                                        "pet:cursor",
                                        api::pet::CursorPosition {
                                            x: logical_x,
                                            y: logical_y,
                                        },
                                    );

                                    let mut is_over_solid = false;
                                    if let Ok(rects) = rects_arc.lock() {
                                        for r in rects.iter() {
                                            if logical_x >= r.x
                                                && logical_y >= r.y
                                                && logical_x <= (r.x + r.width)
                                                && logical_y <= (r.y + r.height)
                                            {
                                                is_over_solid = true;
                                                break;
                                            }
                                        }
                                    }

                                    if is_over_solid {
                                        if was_ignored {
                                            let _ = window.set_ignore_cursor_events(false);
                                            was_ignored = false;
                                        }
                                    } else {
                                        if !was_ignored {
                                            let _ = window.set_ignore_cursor_events(true);
                                            was_ignored = true;
                                        }
                                    }
                                }
                            }
                        }
                    });
                }

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                utils::log_bridge::get_log_history,
                utils::log_bridge::open_log_window,
                utils::log_bridge::is_log_window_open,
                api::plugins::plugin_list,
                api::plugins::plugin_set_enabled,
                api::plugins::plugin_save_config,
                api::plugins::plugin_reload,
                api::plugins::plugin_delete,
                api::plugins::plugin_resources,
                api::plugins::plugin_resource_hide,
                api::plugins::plugin_resource_restore,
                api::plugins::plugin_resource_keep,
                api::plugins::import_plugin_from_path,
                api::plugins::cancel_plugin_import,
                api::settings::get_settings_tree,
                api::settings::save_settings,
                api::settings::get_setting_by_key,
                api::settings::select_file,
                api::settings::list_llm_providers,
                api::settings::save_llm_provider,
                api::settings::delete_llm_provider,
                #[cfg(target_os = "windows")]
                api::settings::set_hdr_mode,
                api::settings::set_llm_role,
                api::settings::switch_llm,
                api::settings::test_llm_provider,
                api::settings::list_llm_models,
                api::codex::codex_auth_status,
                api::codex::codex_start_login,
                api::codex::codex_poll_login,
                api::codex::codex_logout,
                api::codex::codex_get_quota,
                api::font::list_system_fonts,
                api::font::import_font,
                api::font::list_imported_fonts,
                api::font::delete_imported_font,
                api::character::get_character_list,
                api::character::get_role_info,
                api::character::get_role_settings,
                api::character::get_character_file,
                api::character::get_avatar_file,
                api::character::select_clothes,
                api::character::update_role_settings,
                api::character::delete_character,
                api::character::open_characters_folder,
                api::live2d::import_live2d,
                api::live2d::get_live2d_file,
                api::live2d::inspect_live2d,
                api::background::get_background_list,
                api::background::get_background_file,
                api::background::upload_background_image,
                api::background::open_backgrounds_folder,
                api::scene::list_scenes,
                api::scene::create_scene,
                api::scene::update_scene,
                api::scene::delete_scene,
                api::scene::select_scene,
                api::scene::set_scene_awareness,
                api::music::get_music_list,
                api::music::get_music_file,
                api::music::upload_music,
                api::music::delete_music,
                api::music::save_bgm_state,
                api::locale::get_locale_messages,
                api::ambient::get_ambient_list,
                api::ambient::upload_ambient,
                api::ambient::delete_ambient,
                api::ambient::save_ambient_state,
                api::asset::get_asset_base64,
                api::asset::get_voice_audio,
                api::game::init_game,
                api::game::select_character,
                api::game::clear_conversation,
                api::game::reactivate_tts,
                api::game::clear_tts_cache,
                api::game::update_voice_lang,
                api::game::get_tts_cache_info,
                api::game::add_role_to_scene,
                api::game::remove_role_from_scene,
                api::game::notify_player_entry,
                api::chat::send_chat_message,
                api::chat::rollback_conversation,
                api::chat::generate_line_voice,
                api::chat::feed_image,
                api::chat::feed_text,
                api::screenshot::start_screenshot,
                api::screenshot::get_overlay_data,
                api::screenshot::confirm_screenshot,
                api::screenshot::cancel_screenshot,
                api::save::list_saves,
                api::save::create_save,
                api::save::load_save,
                api::save::update_save,
                api::save::delete_save,
                api::save::update_save_title,
                api::save::save_screenshot,
                api::save::capture_main_window_screenshot,
                api::script::list_scripts,
                api::script::list_standalone_scripts,
                api::script::start_script,
                api::script::script_submit_input,
                api::script::script_submit_choice,
                api::script_editor::editor_get_schema,
                api::script_editor::editor_list_scripts,
                api::script_editor::editor_read_script,
                api::script_editor::editor_read_chapter,
                api::script_editor::editor_validate_script,
                api::script_editor::editor_write_chapter,
                api::script_editor::editor_write_story_config,
                api::script_editor::editor_create_chapter,
                api::script_editor::editor_delete_chapter,
                api::script_editor::editor_delete_character,
                api::script_editor::editor_create_script,
                api::script_editor::editor_delete_script,
                api::script_editor::editor_upload_asset,
                api::script_editor::editor_upload_editor_bg,
                api::script_editor::editor_upload_editor_bg_data,
                api::script_editor::editor_create_character,
                api::script_editor::editor_list_global_assets,
                api::script_editor::editor_list_asset_files,
                api::script_editor::editor_delete_asset,
                api::script_editor::editor_rescan_scripts,
                api::script_editor::editor_start_preview,
                api::script_editor::editor_preview_readiness,
                api::script_editor::editor_list_global_characters,
                api::script_editor::editor_import_global_character,
                api::script_editor::editor_stop_preview,
                api::script_editor::editor_open_script_folder,
                api::script_editor::agent::editor_agent_get_settings,
                api::script_editor::agent::editor_agent_save_settings,
                api::script_editor::agent::editor_agent_get_default_dirs,
                api::script_editor::agent::editor_agent_list_skills,
                api::script_editor::agent::editor_agent_read_skill,
                api::script_editor::agent::editor_agent_create_conversation,
                api::script_editor::agent::editor_agent_list_conversations,
                api::script_editor::agent::editor_agent_delete_conversation,
                api::script_editor::agent::editor_agent_rename_conversation,
                api::script_editor::agent::editor_agent_get_messages,
                api::script_editor::agent::editor_agent_clear_conversation,
                api::script_editor::agent::editor_agent_start_chat,
                api::script_editor::agent::editor_agent_stop_chat,
                api::script_editor::agent::editor_agent_rewind,
                api::script_editor::agent::editor_agent_resolve_approval,
                api::pet::update_solid_regions,
                api::pet::set_pet_mode,
                api::schedule::get_schedules,
                api::schedule::save_schedules,
                api::schedule::reload_proactive_system,
                api::proactive_set_can_deliver,
                api::tool_settings::get_tool_settings,
                api::tool_settings::get_tool_runtime_info,
                api::tool_settings::save_tool_settings,
                api::tool_settings::test_web_search,
                api::tool_settings::get_tool_elevation_status,
                api::tool_settings::restart_tool_process_as_admin,
                api::tool_settings::resolve_command_approval,
                api::tool_settings::resolve_file_change_approval,
                api::tool_settings::resolve_file_delete_approval,
                api::achievement::get_achievement_list,
                api::achievement::unlock_achievement,
                api::adventure::list_character_adventures,
                api::adventure::list_all_adventures,
                api::adventure::start_adventure,
                api::adventure::check_adventure_unlocks,
                api::adventure::reset_adventure,
                api::workshop::fetch_discussions,
                resource_sync::check_resource_sync,
                resource_sync::apply_resource_sync,
                resource_sync::get_data_version,
                lan_sync::lan_sync_start_server,
                lan_sync::lan_sync_stop_server,
                lan_sync::lan_sync_scan_peers,
                lan_sync::lan_sync_plan_push,
                lan_sync::lan_sync_execute_push,
                lan_sync::lan_sync_plan_pull,
                lan_sync::lan_sync_execute_pull,
                lan_sync::lan_sync_restart,
                cast::cast_open_window,
                cast::cast_close_window,
                cast::cast_start,
                cast::cast_stop,
                cast::cast_get_status,
                cast::cast_get_snapshot,
                cast::cast_emit_mirror,
                cast::cast_get_mirror,
                cast::cast_play_voice,
                utils::cpu_perf::get_cpu_info,
                utils::cpu_perf::redetect_cpu,
                utils::gpu_perf::get_gpu_info,
                utils::gpu_perf::redetect_gpu,
                utils::gpu_perf::grade_active_gpu,
                api::role_archive::import_role,
                api::role_archive::import_role_from_path,
                api::role_archive::cancel_role_import,
                api::role_archive::rescan_roles,
                api::role_archive::export_role,
                api::role_archive::export_role_to_path,
                ai_service::tts::local::tts_local_status,
                ai_service::tts::local::tts_local_list_catalog,
                ai_service::tts::local::tts_local_list_installed,
                ai_service::tts::local::tts_local_import_from_path,
                ai_service::tts::local::tts_local_download,
                ai_service::tts::local::tts_local_delete_voice,
                ai_service::tts::local::tts_local_delete_deberta,
                ai_service::tts::local::tts_local_import_style_vectors,
                ai_service::tts::local::tts_local_synthesize_preview,
                ai_service::tts::local::tts_local_get_enabled,
                ai_service::tts::cloud::commands::cosyvoice_get_config,
                ai_service::tts::cloud::commands::cosyvoice_save_api_key,
                ai_service::tts::cloud::commands::cosyvoice_create_voice,
                ai_service::tts::cloud::commands::cosyvoice_voice_status,
                ai_service::tts::cloud::commands::cosyvoice_list_voices,
                ai_service::tts::cloud::commands::cosyvoice_delete_voice,
                ai_service::tts::cloud::commands::cosyvoice_synthesize_preview,
                ai_service::tts::local::tts_local_set_enabled,
                ai_service::tts::local::tts_local_get_device,
                ai_service::tts::local::tts_local_list_devices,
                ai_service::tts::local::tts_local_set_device,
                api::asr::asr_start_listening,
                api::asr::asr_stop_listening,
                api::asr::asr_vad_process_chunk,
                api::asr::asr_recognize_wav,
                api::asr::asr_recognize_wav_stream,
                api::asr::asr_cancel,
                api::asr::asr_list_providers,
                api::asr::asr_list_models,
                api::asr::asr_get_settings,
                api::asr::asr_set_settings,
                api::asr::asr_get_status,
                api::asr::asr_test_provider,
                api::asr::asr_start_streaming,
                api::asr::asr_stream_audio_chunk,
                api::asr::asr_stop_streaming,
                api::asr::asr_cancel_streaming,
                exit_app,
            ])
            .run(context)
            .expect("error while running tauri application");
    }
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}
