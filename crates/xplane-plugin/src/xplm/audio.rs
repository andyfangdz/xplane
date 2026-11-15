//! `XPLMSound.h`: calls on an FMOD channel X-Plane's audio playback returned.

use xplane_sdk_sys as xp;

/// An FMOD channel X-Plane's audio calls take, as a plain `usize`. FMOD may
/// recycle a channel once its sound ends, so liveness is the caller's
/// contract and these calls are `unsafe fn`.
pub type Channel = usize;

/// `XPLMStopAudio(channel)`.
///
/// # Safety
/// `channel` is a live FMOD channel from `XPLMPlayPCMOnBus`.
pub unsafe fn stop_audio(channel: Channel) -> i32 {
    // SAFETY: the caller guarantees the channel is live.
    unsafe { xp::XPLMStopAudio(channel as *mut xp::FMOD_CHANNEL) }
}

/// `XPLMSetAudioFadeDistance(channel, min, max)`.
///
/// # Safety
/// As [`stop_audio`].
pub unsafe fn set_audio_fade_distance(channel: Channel, min: f32, max: f32) -> i32 {
    // SAFETY: the caller guarantees the channel is live; plain values otherwise.
    unsafe { xp::XPLMSetAudioFadeDistance(channel as *mut xp::FMOD_CHANNEL, min, max) }
}

/// `XPLMSetAudioVolume(channel, volume)`.
///
/// # Safety
/// As [`stop_audio`].
pub unsafe fn set_audio_volume(channel: Channel, volume: f32) -> i32 {
    // SAFETY: the caller guarantees the channel is live; plain value otherwise.
    unsafe { xp::XPLMSetAudioVolume(channel as *mut xp::FMOD_CHANNEL, volume) }
}

/// `XPLMSetAudioPitch(channel, pitch_hz)`.
///
/// # Safety
/// As [`stop_audio`].
pub unsafe fn set_audio_pitch(channel: Channel, pitch_hz: f32) -> i32 {
    // SAFETY: the caller guarantees the channel is live; plain value otherwise.
    unsafe { xp::XPLMSetAudioPitch(channel as *mut xp::FMOD_CHANNEL, pitch_hz) }
}

/// `XPLMSetAudioPosition(channel, position, velocity)`.
///
/// # Safety
/// As [`stop_audio`].
pub unsafe fn set_audio_position(
    channel: Channel,
    position: &xp::FMOD_VECTOR,
    velocity: &xp::FMOD_VECTOR,
) -> i32 {
    // SAFETY: the caller guarantees the channel is live; X-Plane only reads the two borrowed
    // vectors despite the `*mut` in its signature.
    unsafe {
        xp::XPLMSetAudioPosition(
            channel as *mut xp::FMOD_CHANNEL,
            std::ptr::from_ref(position).cast_mut(),
            std::ptr::from_ref(velocity).cast_mut(),
        )
    }
}

/// `XPLMSetAudioCone(channel, inside, outside, outside_volume, orientation)`.
///
/// # Safety
/// As [`stop_audio`].
pub unsafe fn set_audio_cone(
    channel: Channel,
    inside: f32,
    outside: f32,
    outside_volume: f32,
    orientation: &xp::FMOD_VECTOR,
) -> i32 {
    // SAFETY: the caller guarantees the channel is live; X-Plane only reads the borrowed vector
    // despite the `*mut` in its signature.
    unsafe {
        xp::XPLMSetAudioCone(
            channel as *mut xp::FMOD_CHANNEL,
            inside,
            outside,
            outside_volume,
            std::ptr::from_ref(orientation).cast_mut(),
        )
    }
}
