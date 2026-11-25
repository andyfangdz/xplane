//! `XPLMSound.h`: playback of an in-memory buffer and calls on the FMOD
//! channel it returns.
//!
//! `XPLMSound.h` says a channel "will go away" when its sound completes or is
//! stopped, that the completion callback is "called only once per sound" (also
//! when the play fails), and that after `XPLMStopAudio` the channel "must not
//! be used in any future calls". So whether a channel may be used is not a
//! property of its value: FMOD recycles channel objects. The plain `unsafe fn`
//! calls below leave that to the caller. [`play_pcm_on_bus`] instead keeps a
//! process-wide registry of the sounds it started, keyed by a ticket of its
//! own (not by the channel value), from just before the play call until the
//! completion callback runs or [`stop_live_audio`] retires it; the
//! `*_live_audio` calls make their one XPLM call only for a channel in that
//! registry and otherwise make no call and return `None`. No registry lock is
//! held across an XPLM call or the completion closure.
//!
//! A channel value can come back from a later play while an older ticket
//! still records it: the older sound ended inside the audio system and its
//! completion has not been delivered yet. So a play that returns channel X
//! retires every older ticket recorded with X, and [`stop_live_audio`] on X
//! retires every ticket recorded with X before its call. A retired ticket's
//! completion still runs when X-Plane delivers it and still reclaims its own
//! box (once: X-Plane makes the call once per sound).
//!
//! # Thread
//!
//! `XPLMSound.h` documents that the completion "will be called on the main
//! thread", and every call here must be made from that thread too (the
//! `xplm` module note). Under that contract the registry is exact: between a
//! `*_live_audio` call's liveness check and its XPLM call this plugin makes no
//! other call into X-Plane, so no completion can run in between, and the one
//! completion that can run inside the XPLM call (`XPLMStopAudio` runs it
//! synchronously) belongs to a sound already retired. The lock is released
//! before the XPLM call for that reason: the synchronous completion takes it.
//!
//! A completion delivered on another thread, which the header does not allow,
//! would still leave the registry consistent (each step is one locked
//! update) and would still reclaim its box once. It would not keep the
//! `*_live_audio` calls sound: it could end a sound after the check and before
//! the XPLM call, which would then pass X-Plane a channel it has let go; and
//! the caller's closure would run on that thread, with a panic there logged
//! through `XPLMDebugString` off the main thread. Closing that gap would need
//! the lock held across the XPLM call, which the synchronous completion of
//! `XPLMStopAudio` would deadlock on; this module relies on the documented
//! main thread instead.
//!
//! The completion trampoline runs the caller's closure under
//! [`crate::guard::guard_teardown`], so it runs even after the plugin has
//! faulted: a completion is resource bookkeeping (the caller learns its sound
//! is gone), not plugin behaviour that drives the simulator.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use xplane_sdk_sys as xp;

/// An FMOD channel X-Plane's audio calls take, as a plain `usize`. FMOD may
/// recycle a channel once its sound ends, so liveness is the caller's
/// contract for the `unsafe fn` calls; the `*_live_audio` calls check it.
pub type Channel = usize;

/// Sounds [`play_pcm_on_bus`] started whose completion has not run and that
/// were not stopped: ticket -> channel (0 until the play returns).
static LIVE: LazyLock<Mutex<HashMap<u64, Channel>>> = LazyLock::new(Mutex::default);
static NEXT_TICKET: AtomicU64 = AtomicU64::new(0);

/// The registry, locked for one short Rust-only step (never across an XPLM
/// call or a completion closure). A poisoned lock still holds a consistent
/// map: every step is a single insert, update or remove.
fn live() -> MutexGuard<'static, HashMap<u64, Channel>> {
    LIVE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a playing sound owns until X-Plane's completion callback.
struct Playing<B> {
    ticket: u64,
    buffer: B,
    completion: Box<dyn FnOnce(i32) + Send>,
}

/// The ticket of the live sound on `channel`, if any. The null channel is
/// never live.
fn live_ticket(channel: Channel) -> Option<u64> {
    if channel == 0 {
        return None;
    }
    live()
        .iter()
        .find_map(|(&ticket, &on)| (on == channel).then_some(ticket))
}

/// Records that the play of `ticket` returned `channel`: every other ticket
/// recorded with that (non-null) channel is an older sound whose channel was
/// recycled, so it is retired. `ticket` itself is updated only if its
/// completion has not already removed it.
fn record_channel(live: &mut HashMap<u64, Channel>, ticket: u64, channel: Channel) {
    if channel != 0 {
        live.retain(|&other, &mut on| other == ticket || on != channel);
    }
    if let Some(slot) = live.get_mut(&ticket) {
        *slot = channel;
    }
}

/// Retires every ticket recorded with `channel`; whether there was one. The
/// null channel is never live.
fn retire_channel(live: &mut HashMap<u64, Channel>, channel: Channel) -> bool {
    if channel == 0 {
        return false;
    }
    let before = live.len();
    live.retain(|_, &mut on| on != channel);
    live.len() != before
}

/// `XPLMPlayPCMOnBus(buffer, len, format, hz, channels, looped, bus,
/// completion, ticket)`: plays `buffer` (its length saturated at `i32::MAX`)
/// and returns the channel, which may be null when the play fails.
///
/// The sound owns `buffer` and `completion` until X-Plane's completion
/// callback, which X-Plane makes once per sound on the main thread, also when
/// the play fails; it then calls `completion(status)` under the crate's panic
/// guard (also after a plugin fault: module note) and drops both. Until then,
/// or until a later play returns the same channel value, the channel is live
/// for the `*_live_audio` calls.
pub fn play_pcm_on_bus<B: AsRef<[u8]> + Send + 'static>(
    buffer: B,
    format: i32,
    hz: i32,
    channels: i32,
    looped: i32,
    bus: i32,
    completion: Box<dyn FnOnce(i32) + Send>,
) -> Channel {
    let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
    live().insert(ticket, 0);
    let playing = Box::into_raw(Box::new(Playing {
        ticket,
        buffer,
        completion,
    }));
    // Taken from the boxed buffer, so it stays put until the completion frees the box.
    let (data, len) = {
        // SAFETY: `playing` is the box just leaked; this shared borrow ends before the call.
        let bytes = unsafe { (*playing).buffer.as_ref() };
        (
            bytes.as_ptr(),
            i32::try_from(bytes.len()).unwrap_or(i32::MAX),
        )
    };
    // SAFETY: `data` points at `len` bytes of the leaked box's buffer, which nothing else
    // touches and which stays allocated until `complete::<B>` runs: X-Plane calls it once per
    // sound (also for a failed play), after which it no longer reads the buffer. FMOD only
    // reads the sample data despite the `*mut` in the signature. `complete::<B>` is given the
    // `Playing<B>` box it expects as its refcon.
    let channel = unsafe {
        xp::XPLMPlayPCMOnBus(
            data.cast_mut().cast::<c_void>(),
            len,
            format,
            hz,
            channels,
            looped,
            bus,
            Some(complete::<B>),
            playing.cast::<c_void>(),
        )
    } as Channel;
    // A failed play may already have run the completion, which removed the ticket; older
    // tickets still recorded with this channel are retired either way (module note).
    record_channel(&mut live(), ticket, channel);
    channel
}

/// X-Plane's completion callback for [`play_pcm_on_bus`].
unsafe extern "C" fn complete<B: AsRef<[u8]> + Send + 'static>(refcon: *mut c_void, status: i32) {
    // SAFETY: the refcon is the `Playing<B>` box `play_pcm_on_bus::<B>` leaked, and X-Plane
    // calls this once per sound, so the box is taken back exactly once.
    let playing = unsafe { Box::from_raw(refcon.cast::<Playing<B>>()) };
    finish(*playing, status, &crate::guard::PLUGIN);
}

/// The body of [`complete`]: retires the ticket (a no-op when a recycled
/// channel or a stop already retired it), runs the closure, drops the buffer.
fn finish<B>(playing: Playing<B>, status: i32, boundary: &crate::guard::Boundary) {
    live().remove(&playing.ticket);
    let Playing {
        buffer, completion, ..
    } = playing;
    // Teardown, not `guard`: a completion is resource bookkeeping. Skipping it after a fault
    // would leave the caller's sound state saying the sound still plays, for good. A panic is
    // still contained and reported. `PLUGIN.call_teardown` is `guard::guard_teardown`.
    boundary.call_teardown("audio completion", (), || completion(status));
    drop(buffer);
}

/// [`stop_audio`] for a live channel (module note), retiring every ticket
/// recorded with it before the call; `None` (no call) when none is live.
pub fn stop_live_audio(channel: Channel) -> Option<i32> {
    if !retire_channel(&mut live(), channel) {
        return None;
    }
    // SAFETY: the channel was live (registry checked): its completion had not run and it was
    // not stopped, so X-Plane has not let it go.
    Some(unsafe { stop_audio(channel) })
}

/// The `*_live_audio` setters: each makes its one call only for a channel in
/// the registry (module note). Members: [`set_live_audio_fade_distance`],
/// [`set_live_audio_volume`], [`set_live_audio_pitch`],
/// [`set_live_audio_position`], [`set_live_audio_cone`].
macro_rules! live_calls {
    ($($(#[$attr:meta])* pub fn $name:ident($channel:ident $(, $arg:ident: $ty:ty)*) = $unsafe_fn:ident;)+) => {$(
        $(#[$attr])*
        pub fn $name($channel: Channel $(, $arg: $ty)*) -> Option<i32> {
            live_ticket($channel)?;
            // SAFETY: a live call (macro note): the channel is in the registry, so its completion
            // has not run and it was not stopped, which is the whole contract of the call; the
            // other arguments are plain values or borrows the call only reads.
            Some(unsafe { $unsafe_fn($channel $(, $arg)*) })
        }
    )+};
}

live_calls! {
    /// [`set_audio_fade_distance`] for a live channel, or `None`.
    pub fn set_live_audio_fade_distance(channel, min: f32, max: f32) = set_audio_fade_distance;
    /// [`set_audio_volume`] for a live channel, or `None`.
    pub fn set_live_audio_volume(channel, volume: f32) = set_audio_volume;
    /// [`set_audio_pitch`] for a live channel, or `None`.
    pub fn set_live_audio_pitch(channel, pitch_hz: f32) = set_audio_pitch;
    /// [`set_audio_position`] for a live channel, or `None`.
    pub fn set_live_audio_position(
        channel,
        position: &xp::FMOD_VECTOR,
        velocity: &xp::FMOD_VECTOR
    ) = set_audio_position;
    /// [`set_audio_cone`] for a live channel, or `None`.
    pub fn set_live_audio_cone(
        channel,
        inside: f32,
        outside: f32,
        outside_volume: f32,
        orientation: &xp::FMOD_VECTOR
    ) = set_audio_cone;
}

caller_handle_calls! {
    /// `XPLMStopAudio(channel)`.
    ///
    /// # Safety
    /// `channel` is a live FMOD channel from `XPLMPlayPCMOnBus`.
    pub unsafe fn stop_audio(channel as *mut xp::FMOD_CHANNEL) -> i32 = XPLMStopAudio;
    /// `XPLMSetAudioFadeDistance(channel, min, max)`.
    ///
    /// # Safety
    /// As [`stop_audio`].
    pub unsafe fn set_audio_fade_distance(channel as *mut xp::FMOD_CHANNEL, min: f32, max: f32)
        -> i32 = XPLMSetAudioFadeDistance;
    /// `XPLMSetAudioVolume(channel, volume)`.
    ///
    /// # Safety
    /// As [`stop_audio`].
    pub unsafe fn set_audio_volume(channel as *mut xp::FMOD_CHANNEL, volume: f32) -> i32 =
        XPLMSetAudioVolume;
    /// `XPLMSetAudioPitch(channel, pitch_hz)`.
    ///
    /// # Safety
    /// As [`stop_audio`].
    pub unsafe fn set_audio_pitch(channel as *mut xp::FMOD_CHANNEL, pitch_hz: f32) -> i32 =
        XPLMSetAudioPitch;
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

#[cfg(test)]
mod tests {
    // Only the registry and the completion body are exercised; no SDK call is made.
    use super::*;
    use crate::guard::Boundary;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    #[test]
    fn a_play_that_returns_a_recorded_channel_retires_the_older_tickets() {
        let mut live = HashMap::from([(1, 0x40), (2, 0x40), (3, 0x80), (4, 0)]);
        record_channel(&mut live, 4, 0x40);
        assert_eq!(live, HashMap::from([(3, 0x80), (4, 0x40)]));
        // A failed play (null channel) retires nothing.
        live.insert(5, 0);
        record_channel(&mut live, 5, 0);
        assert_eq!(live.len(), 3);
        // A play whose completion already removed its ticket still retires stale ones.
        record_channel(&mut live, 6, 0x80);
        assert_eq!(live, HashMap::from([(4, 0x40), (5, 0)]));
    }

    #[test]
    fn a_stop_retires_every_ticket_recorded_with_the_channel() {
        let mut live = HashMap::from([(1, 0x40), (2, 0x40), (3, 0x80), (4, 0)]);
        assert!(retire_channel(&mut live, 0x40));
        assert_eq!(live, HashMap::from([(3, 0x80), (4, 0)]));
        assert!(!retire_channel(&mut live, 0x40));
        assert!(!retire_channel(&mut live, 0));
        assert_eq!(live.len(), 2);
    }

    #[test]
    fn a_retired_ticket_still_completes_and_drops_its_buffer() {
        let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
        let buffer = Arc::new([0u8; 4]);
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let playing = Playing {
            ticket,
            buffer: buffer.clone(),
            completion: Box::new(move |status| {
                seen.fetch_add(usize::try_from(status).unwrap(), Ordering::SeqCst);
            }),
        };
        // Retired by a play that got the same channel back, before the completion arrives.
        {
            let mut live = live();
            live.insert(ticket, 0x40);
            record_channel(&mut live, ticket + 1_000_000, 0x40);
            assert!(!live.contains_key(&ticket));
        }
        finish(playing, 3, &Boundary::new());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(Arc::strong_count(&buffer), 1);
        assert!(!live().contains_key(&ticket));
    }

    #[test]
    fn a_completion_still_runs_after_a_fault() {
        let boundary = Boundary::new();
        boundary.set_logger(|_| {});
        boundary.report_fault("test", "faulted before the completion");
        assert!(boundary.faulted());
        let ran = Arc::new(AtomicBool::new(false));
        let seen = ran.clone();
        let playing = Playing {
            ticket: NEXT_TICKET.fetch_add(1, Ordering::Relaxed),
            buffer: [0u8; 2],
            completion: Box::new(move |_| seen.store(true, Ordering::SeqCst)),
        };
        finish(playing, 0, &boundary);
        assert!(ran.load(Ordering::SeqCst));
    }
}
