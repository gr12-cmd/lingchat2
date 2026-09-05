//! ASR 会话编排：互斥锁 + 取消令牌 + vad / providers 协调。

#[cfg(not(target_os = "android"))]
mod impl_ {
    use std::collections::HashMap;
    use std::sync::Arc;

    use serde::Serialize;
    use tauri::Emitter;
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    use super::super::error::AsrError;
    use super::super::provider::{AsrProvider, AsrResult};
    use super::super::provider_stream::{self, StreamCommand};
    use super::super::vad::AsrVad;

    /// ASR 会话来源。两种触发源共享同一会话生命周期。
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum AsrSource {
        Button,
        Auto,
    }

    impl AsrSource {
        pub fn from_str(s: &str) -> Option<Self> {
            match s {
                "button" => Some(Self::Button),
                "auto" => Some(Self::Auto),
                _ => None,
            }
        }
    }

    /// 流式会话句柄：命令通道 + 注册 provider id。
    #[derive(Clone)]
    pub struct StreamHandle {
        pub provider_id: String,
        pub tx: tokio::sync::mpsc::UnboundedSender<StreamCommand>,
    }

    /// ASR 会话编排器。
    pub struct AsrSession {
        pub vad: Arc<AsrVad>,
        pub providers: Mutex<HashMap<String, Arc<dyn AsrProvider>>>,
        pub active_source: Mutex<Option<AsrSource>>,
        pub cancel_token: Mutex<CancellationToken>,
        pub stream: Mutex<Option<StreamHandle>>,
        pub lock: Mutex<()>,
    }

    impl AsrSession {
        pub fn new(vad: Arc<AsrVad>, providers: HashMap<String, Arc<dyn AsrProvider>>) -> Self {
            Self {
                vad,
                providers: Mutex::new(providers),
                active_source: Mutex::new(None),
                cancel_token: Mutex::new(CancellationToken::new()),
                stream: Mutex::new(None),
                lock: Mutex::new(()),
            }
        }

        pub async fn start(&self, source: AsrSource) -> Result<(), AsrError> {
            let _guard = self.lock.lock().await;
            let mut active = self.active_source.lock().await;
            if active.is_some() {
                return Err(AsrError::SessionBusy);
            }
            *active = Some(source);
            self.vad.reset().await;
            Ok(())
        }

        pub async fn stop(&self, source: AsrSource) -> Result<(), AsrError> {
            let mut active = self.active_source.lock().await;
            if *active != Some(source) {
                return Err(AsrError::Canceled);
            }
            *active = None;
            Ok(())
        }

        pub async fn vad_process_chunk(
            &self,
            app: &tauri::AppHandle,
            pcm: Vec<f32>,
        ) -> Result<(), AsrError> {
            let active = self.active_source.lock().await;
            if *active != Some(AsrSource::Auto) {
                return Ok(());
            }
            drop(active);
            self.vad.process_chunk(app, &pcm).await.map(|_| ())
        }

        pub async fn recognize_wav(
            &self,
            provider_id: String,
            wav_bytes: Vec<u8>,
            language_hint: Option<String>,
        ) -> Result<AsrResult, AsrError> {
            let provider = self
                .providers
                .lock()
                .await
                .get(&provider_id)
                .cloned()
                .ok_or_else(|| AsrError::ProviderNotFound(provider_id.clone()))?;
            self.recognize_wav_with(provider.clone(), wav_bytes, language_hint.as_deref())
                .await
        }

        pub async fn recognize_wav_with(
            &self,
            provider: Arc<dyn AsrProvider>,
            wav_bytes: Vec<u8>,
            language_hint: Option<&str>,
        ) -> Result<AsrResult, AsrError> {
            let cancel_child = self.cancel_token.lock().await.clone().child_token();
            tokio::select! {
                result = provider.recognize(wav_bytes, language_hint) => result,
                _ = cancel_child.cancelled() => Err(AsrError::Canceled),
            }
        }

        pub async fn current_source(&self) -> Option<AsrSource> {
            *self.active_source.lock().await
        }

        pub async fn cancel(&self) {
            let mut token = self.cancel_token.lock().await;
            token.cancel();
            *token = CancellationToken::new();
        }

        pub async fn start_streaming(
            &self,
            app: &tauri::AppHandle,
            provider_id: &str,
            endpoint: String,
            api_key: String,
            model: String,
            language_hint: Option<String>,
        ) -> Result<(), AsrError> {
            if self.stream.lock().await.take().is_some() {
                tracing::warn!("[ASR/stream] 丢弃残留流式会话句柄");
            }
            let app_handle = app.clone();
            let on_partial = std::sync::Arc::new(move |text: &str| {
                let _ = app_handle.emit("asr://stream_partial", text.to_string());
            });
            let tx =
                provider_stream::start_streaming(on_partial, endpoint, api_key, model, language_hint)
                    .await?;
            *self.stream.lock().await = Some(StreamHandle {
                provider_id: provider_id.to_string(),
                tx,
            });
            Ok(())
        }

        pub async fn stream_audio_chunk(&self, pcm: Vec<f32>) -> Result<(), AsrError> {
            let active = self.active_source.lock().await;
            if active.is_none() {
                return Ok(());
            }
            drop(active);
            let handle = self.stream.lock().await.clone();
            match handle {
                Some(h) => {
                    h.tx.send(StreamCommand::Audio(pcm))
                        .map_err(|_| AsrError::Canceled)
                }
                None => Err(AsrError::EngineLoadFailed("流式会话未启动".into())),
            }
        }

        pub async fn stop_streaming(&self) -> Result<AsrResult, AsrError> {
            let handle = self.stream.lock().await.take();
            let Some(h) = handle else {
                return Err(AsrError::Canceled);
            };
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            h.tx.send(StreamCommand::Stop { reply: reply_tx })
                .map_err(|_| AsrError::Canceled)?;
            let result = tokio::time::timeout(std::time::Duration::from_secs(30), reply_rx)
                .await
                .map_err(|_| AsrError::ProviderTimeout("qwen-asr".into()))?
                .map_err(|_| AsrError::ProviderTimeout("qwen-asr".into()))?;
            let text = result?.text;
            Ok(AsrResult {
                text,
                language: None,
                confidence: None,
                provider_id: h.provider_id,
            })
        }

        pub async fn cancel_stream(&self) {
            let mut g = self.stream.lock().await;
            if let Some(h) = g.take() {
                let _ = h.tx.send(StreamCommand::Abort);
            }
        }
    }
}

// Android 上 ASR session 是一个空桩
#[cfg(target_os = "android")]
mod impl_ {
    // 空实现，所有调用返回错误或空
}

// 公共 API 重新导出
#[cfg(not(target_os = "android"))]
pub use impl_::*;

// Android 上所有类型都是空桩，调用会 panic 或返回错误
#[cfg(target_os = "android")]
pub use android_stub::*;

#[cfg(target_os = "android")]
mod android_stub {
    use super::super::error::AsrError;
    use super::super::provider::AsrResult;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum AsrSource {
        Button,
        Auto,
    }

    impl AsrSource {
        pub fn from_str(s: &str) -> Option<Self> {
            match s {
                "button" => Some(Self::Button),
                "auto" => Some(Self::Auto),
                _ => None,
            }
        }
    }

    #[derive(Clone)]
    pub struct StreamHandle;

    pub struct AsrSession;

    impl AsrSession {
        pub async fn start(&self, _source: AsrSource) -> Result<(), AsrError> {
            Err(AsrError::EngineLoadFailed("ASR not supported on Android".into()))
        }

        pub async fn stop(&self, _source: AsrSource) -> Result<(), AsrError> {
            Err(AsrError::Canceled)
        }

        pub async fn vad_process_chunk(&self, _app: &tauri::AppHandle, _pcm: Vec<f32>) -> Result<(), AsrError> {
            Err(AsrError::EngineLoadFailed("ASR not supported on Android".into()))
        }

        pub async fn recognize_wav(
            &self,
            _provider_id: String,
            _wav_bytes: Vec<u8>,
            _language_hint: Option<String>,
        ) -> Result<AsrResult, AsrError> {
            Err(AsrError::EngineLoadFailed("ASR not supported on Android".into()))
        }

        pub async fn current_source(&self) -> Option<AsrSource> {
            None
        }

        pub async fn cancel(&self) {}

        pub async fn start_streaming(
            &self,
            _app: &tauri::AppHandle,
            _provider_id: &str,
            _endpoint: String,
            _api_key: String,
            _model: String,
            _language_hint: Option<String>,
        ) -> Result<(), AsrError> {
            Err(AsrError::EngineLoadFailed("ASR not supported on Android".into()))
        }

        pub async fn stream_audio_chunk(&self, _pcm: Vec<f32>) -> Result<(), AsrError> {
            Err(AsrError::EngineLoadFailed("ASR not supported on Android".into()))
        }

        pub async fn stop_streaming(&self) -> Result<AsrResult, AsrError> {
            Err(AsrError::EngineLoadFailed("ASR not supported on Android".into()))
        }

        pub async fn cancel_stream(&self) {}
    }
}
