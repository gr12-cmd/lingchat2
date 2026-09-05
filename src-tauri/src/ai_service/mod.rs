//! AI 服务模块（ASR、TTS、情感分析等）

#[cfg(not(target_os = "android"))]
pub mod asr;
#[cfg(not(target_os = "android"))]
pub mod emotion;
#[cfg(not(target_os = "android"))]
pub mod tts;

// Android 上不编译 AI 服务
#[cfg(target_os = "android")]
pub mod asr {
    // 空桩，让依赖它的代码能看到这个模块存在，但不提供任何功能
}
#[cfg(target_os = "android")]
pub mod emotion {}
#[cfg(target_os = "android")]
pub mod tts {}
