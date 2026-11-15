//! The exact-call layer: one XPLM call per function, with the arguments given.
//!
//! The rest of this crate offers owned, ergonomic types such as
//! [`crate::DataRef`], [`crate::Command`], [`crate::PluginMenu`],
//! [`crate::Window`] and [`crate::TerrainProbe`], which choose their own
//! defaults, combine calls and clean up on drop. This module is the layer
//! underneath for code that needs to make a particular sequence of SDK calls
//! and nothing else, for example when a known call sequence must be
//! reproduced exactly. Each function makes exactly one SDK call (or the
//! documented pair noted on it), with the arguments it is given: no call is
//! added, skipped or reordered, so code that calls XPLM with a null dataref
//! still makes that call. The items are grouped by SDK header in private
//! submodules and re-exported here flat.
//!
//! # Thread
//!
//! XPLM belongs to X-Plane's main (plugin) thread. Like the rest of this
//! crate, every function here must be called from that thread: from a plugin
//! entry point, flight loop, draw, command, menu, window or dataref callback.
//! That is the convention of the whole SDK boundary and is not checked here,
//! except that the registries of terrain probes, objects and instances are
//! per thread, so a handle made on one thread is unknown on another and its
//! calls panic there.
//!
//! # Handles
//!
//! [`DataRef`] and [`CommandRef`] may hold the null handle, because a lookup
//! that finds nothing is commonly kept as a `usize` 0 and still passed to the
//! get/set calls, which `XPLMDataAccess.h` documents as checking for null. The
//! other dataref and command calls (`types`, `writable`, `good`, `once`,
//! `begin`, `end`) have no such promise, so they panic on the null handle.
//! A non-null value is only ever one returned by `XPLMFindDataRef` /
//! `XPLMFindCommand` / `XPLMCreateCommand`; X-Plane never frees dataref or
//! command objects, so such a handle stays valid for the process. Code that
//! stores the raw `usize` gets the handle back with `from_raw`, which accepts
//! only 0 or a value this module's lookup returned (a process-wide registry of
//! found handles checks it), so no safe call can hand XPLM a forged pointer.
//! [`SharedMenu`] does the same for X-Plane's own Plugins and Aircraft menus.
//!
//! Menus and windows a plugin creates can be destroyed, so the item and
//! window calls take [`LiveMenu`] / [`LiveWindow`], borrowed from the owning
//! [`crate::CallbackMenu`] / [`crate::CallbackWindow`]; a raw id needs the
//! `unsafe` `from_raw`. Neither is ever null: the SDK does not promise a null
//! check for those calls (except `XPLMTakeKeyboardFocus`, documented to take
//! null for X-Plane). Terrain probes, loaded objects and object instances are
//! plain integers; a per-thread registry of the ones not yet given back checks
//! each call, so those calls are safe and panic on a handle that is unknown,
//! already given back or from another thread. Audio channels and avionics
//! devices have lifetimes this module cannot see, so their calls are
//! `unsafe fn`, except the `*_live_audio` calls, which check a process-wide
//! registry of the sounds [`play_pcm_on_bus`] started.
//!
//! Integer identifiers that are not addresses (plugin ids, nav refs, flight
//! plan and entry indices, menu item indices, probe types, positioning modes,
//! texture numbers) are passed through unchanged, and X-Plane looks them up in
//! its own tables. What the headers establish differs by kind:
//!
//! - Nav refs: `XPLMNavigation.h` documents `XPLM_NAV_NOT_FOUND` from
//!   `XPLMGetNextNavAid` for an invalid nav ref. Established for that call.
//! - Flight-plan entry indices (`XPLMClearFMSFlightPlanEntry`,
//!   `XPLMSetFMSFlightPlanEntryInfo`, `XPLMSetDirectToFMSFlightPlanEntry`,
//!   `XPLMSetDisplayedFMSFlightPlanEntry`,
//!   `XPLMSetDestinationFMSFlightPlanEntry`, the entry-info reads): the header
//!   gives the valid range only through `XPLMCountFMSFlightPlanEntries` and
//!   defines no rejection for an index outside it. Trusted, not established:
//!   this module assumes X-Plane bounds the index itself.
//! - Plugin ids: the header returns `XPLM_NO_PLUGIN_ID` from the lookups but
//!   says nothing about passing an unknown id to `XPLMIsPluginEnabled` or
//!   `XPLMGetPluginInfo`. Trusted, not established.
//! - Menu item indices: `XPLMMenus.h` defines no rejection; X-Plane has been
//!   observed to log an out-of-range index. Trusted, not established.
//! - Probe types, positioning modes and texture numbers: values X-Plane uses
//!   as selectors or GL names; no header defines a rejection. Trusted, not
//!   established.
//!
//! "Trusted" means the safe wrapper relies on X-Plane handling the value
//! without undefined behaviour although no header promises it; these are
//! open gaps, listed here so they are not mistaken for guarantees.
//!
//! # Array calls
//!
//! A slice's length is passed as the SDK's `int` count, saturated at
//! `i32::MAX` so it never goes negative and never exceeds the slice. The
//! `offset` is passed through unchanged, negative values included: it indexes
//! the dataref's own array, not the caller's buffer, and X-Plane (or the
//! accessor of the plugin that owns the dataref) bounds it; it cannot move
//! where X-Plane writes or reads in the buffer, which is always the first
//! `count` elements. Calls that take a fixed-size output buffer without a
//! length panic when the slice is shorter than the size the SDK header asks
//! for, and calls that take a struct with a `structSize` field panic when
//! that field is negative or larger than the struct.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::thread::LocalKey;

/// Wrappers for the class of XPLM calls that take only plain numbers (or
/// nothing) and return a plain number or a handle value the wrapper only
/// records: no pointer argument, nothing whose validity or lifetime a caller
/// must keep. Each value is either an integer identifier X-Plane looks up in
/// its own tables (module note: nav refs, whose rejection `XPLMGetNextNavAid`
/// documents; flight plan entry indices and plugin ids, whose rejection no
/// header documents and which are trusted, not established) or used by
/// X-Plane only as a number (headings, coordinates, altitudes, GL flags and
/// texture names). Members:
/// `XPLMGetFirstNavAid`, `XPLMGetNextNavAid`, `XPLMGetGPSDestination`,
/// `XPLMCountFMSFlightPlanEntries`, `XPLMClearFMSFlightPlanEntry`,
/// `XPLMSetFMSFlightPlanEntryInfo`, `XPLMSetDirectToFMSFlightPlanEntry`,
/// `XPLMSetDisplayedFMSFlightPlanEntry`, `XPLMGetDestinationFMSFlightPlanEntry`,
/// `XPLMSetDestinationFMSFlightPlanEntry`, `XPLMDegTrueToDegMagnetic`,
/// `XPLMGetMagneticVariation`, `XPLMSetGraphicsState`, `XPLMBindTexture2d`,
/// `XPLMDrawTranslucentDarkBox`, `XPLMGetCycleNumber`, `XPLMGetMyID`,
/// `XPLMIsPluginEnabled`, `XPLMCreateProbe`, `XPLMFindPluginsMenu`,
/// `XPLMFindAircraftMenu`.
macro_rules! value_calls {
    ($($(#[$attr:meta])* $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? = $sdk:ident;)+) => {$(
        $(#[$attr])*
        $vis fn $name($($arg: $ty),*) $(-> $ret)? {
            // SAFETY: a value call (macro note): only plain numbers cross the boundary, which
            // X-Plane looks up in its own tables (bounded by X-Plane: documented for nav refs,
            // trusted for the other ids, module note) or uses only as numbers; no memory is
            // passed or retained.
            unsafe { ::xplane_sdk_sys::$sdk($($arg),*) }
        }
    )+};
}

/// Wrappers for the class of XPLM calls that take one NUL-terminated string,
/// which X-Plane reads during the call only (copying what it keeps), plus
/// plain numbers, and return a number or a handle value the wrapper only
/// stores: the `&CStr` is NUL-terminated and borrowed for the whole call.
/// Members: `XPLMDebugString`, `XPLMFindDataRef`, `XPLMFindPluginBySignature`,
/// `XPLMEnableFeature`, `XPLMLoadObject`.
macro_rules! str_calls {
    ($($(#[$attr:meta])* $vis:vis fn $name:ident($text:ident: &CStr $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? = $sdk:ident;)+) => {$(
        $(#[$attr])*
        $vis fn $name($text: &::std::ffi::CStr $(, $arg: $ty)*) $(-> $ret)? {
            // SAFETY: a string call (macro note): the string is NUL-terminated and outlives the
            // call, which only reads it; plain numbers otherwise.
            unsafe { ::xplane_sdk_sys::$sdk($text.as_ptr() $(, $arg)*) }
        }
    )+};
}

/// `unsafe fn` wrappers for the class of XPLM calls that pass one handle whose
/// lifetime this module cannot see, plus plain numbers: the caller guarantees
/// the handle is live (each member's `# Safety`), and its value is cast to the
/// SDK's pointer type unchanged. Members: `XPLMStopAudio`,
/// `XPLMSetAudioFadeDistance`, `XPLMSetAudioVolume`, `XPLMSetAudioPitch`,
/// `XPLMSetAvionicsPopupVisible`, `XPLMGetAvionicsBusVoltsRatio`.
macro_rules! caller_handle_calls {
    ($($(#[$attr:meta])* pub unsafe fn $name:ident($handle:ident as $sdk_ty:ty $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? = $sdk:ident;)+) => {$(
        $(#[$attr])*
        pub unsafe fn $name($handle: usize $(, $arg: $ty)*) $(-> $ret)? {
            // SAFETY: a caller-handle call (macro note): the caller guarantees the handle is
            // live; plain numbers otherwise.
            unsafe { ::xplane_sdk_sys::$sdk($handle as $sdk_ty $(, $arg)*) }
        }
    )+};
}

/// Wrappers for the class of XPLM calls that take plain numbers and write
/// their results through out pointers, one per result, and keep none of them:
/// each out pointer is a distinct local of the declared type, live and
/// exclusively borrowed for the call, and the results come back as an array
/// in SDK argument order. Members: `XPLMGetScreenSize`,
/// `XPLMGetScreenBoundsGlobal`, `XPLMGetMouseLocationGlobal`,
/// `XPLMWorldToLocal`, `XPLMLocalToWorld`.
macro_rules! out_calls {
    ($($(#[$attr:meta])* $vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> [$out:ty; $n:literal] = $sdk:ident($($slot:ident),+);)+) => {$(
        $(#[$attr])*
        $vis fn $name($($arg: $ty),*) -> [$out; $n] {
            $(let mut $slot: $out = Default::default();)+
            // SAFETY: an out call (macro note): every out pointer is a distinct local of the type
            // the SDK writes, live for the call; the inputs are plain numbers; nothing is kept.
            unsafe { ::xplane_sdk_sys::$sdk($($arg,)* $(&mut $slot),+) };
            [$($slot),+]
        }
    )+};
}

mod audio;
mod commands;
mod data_access;
mod display;
mod graphics;
pub(crate) mod menus;
mod navigation;
mod plugin;
mod scenery;
mod utilities;

pub use audio::{
    Channel, play_pcm_on_bus, set_audio_cone, set_audio_fade_distance, set_audio_pitch,
    set_audio_position, set_audio_volume, set_live_audio_cone, set_live_audio_fade_distance,
    set_live_audio_pitch, set_live_audio_position, set_live_audio_volume, stop_audio,
    stop_live_audio,
};
pub use commands::CommandRef;
pub(crate) use commands::once_handle;
pub use data_access::DataRef;
pub use display::{
    LiveWindow, all_monitor_bounds_global, avionics_bus_volts_ratio, mouse_location_global,
    screen_bounds_global, screen_size, screen_size_array, set_avionics_popup_visible,
    take_keyboard_focus,
};
pub use graphics::{
    bind_texture_2d, draw_string, draw_translucent_dark_box, generate_texture_numbers,
    local_to_world, set_graphics_state, world_to_local,
};
pub use menus::{LiveMenu, PlainMenu, SharedMenu};
pub use navigation::{
    NavAidInfo, clear_fms_flight_plan_entry, count_fms_flight_plan_entries,
    destination_fms_flight_plan_entry, find_nav_aid, first_nav_aid, gps_destination, nav_aid_info,
    next_nav_aid, set_destination_fms_flight_plan_entry, set_direct_to_fms_flight_plan_entry,
    set_displayed_fms_flight_plan_entry, set_fms_flight_plan_entry_info,
};
pub use plugin::{
    PluginInfo, enable_feature, find_plugin_by_signature, is_plugin_enabled, my_id,
    plugin_file_path, plugin_info,
};
pub use scenery::{
    create_instance, create_probe, degrees_true_to_magnetic, destroy_instance, destroy_probe,
    instance_set_position, load_object, magnetic_variation, probe_terrain_xyz, unload_object,
    weather_at_location,
};
pub use utilities::{
    cycle_number, debug_bytes, debug_string, nth_aircraft_model, prefs_path, system_path,
};

/// A multiply-shift hash for handle addresses. The sets only ever hold values
/// X-Plane returned, so SipHash's flooding resistance buys nothing here and
/// would be most of the cost of `from_raw`.
#[derive(Clone, Copy, Default)]
struct AddressHash(u64);

impl Hasher for AddressHash {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(self.0.rotate_left(8) ^ u64::from(byte));
        }
    }

    fn write_u64(&mut self, value: u64) {
        let mixed = value.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        // Fold the well-mixed high half into the low bits the table indexes by:
        // aligned addresses have zero low bits, and so does their plain product.
        self.0 = mixed ^ (mixed >> 32);
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64)
    }
}

type AddressSet = HashSet<usize, BuildHasherDefault<AddressHash>>;

/// Raw handles that an XPLM lookup returned, per handle kind: a process-wide
/// set, fronted by a per-thread copy of the handles that thread has already
/// confirmed, so the common check takes no lock. A thread that has not seen a
/// handle (or is tearing down its thread-locals) falls back to the global set.
struct Found {
    all: LazyLock<Mutex<AddressSet>>,
    seen: &'static LocalKey<RefCell<AddressSet>>,
}

impl Found {
    const fn new(seen: &'static LocalKey<RefCell<AddressSet>>) -> Self {
        Self {
            all: LazyLock::new(|| Mutex::new(AddressSet::default())),
            seen,
        }
    }

    fn insert(&self, raw: usize) {
        if raw != 0 {
            self.all
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(raw);
            self.remember(raw);
        }
    }

    fn contains(&self, raw: usize) -> bool {
        if raw == 0 {
            return true;
        }
        let known = self
            .seen
            .try_with(|seen| seen.try_borrow().is_ok_and(|seen| seen.contains(&raw)))
            .unwrap_or(false);
        if known {
            return true;
        }
        let found = self
            .all
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&raw);
        if found {
            self.remember(raw);
        }
        found
    }

    /// Add `raw` to this thread's copy, if the thread-local is still usable.
    fn remember(&self, raw: usize) {
        // Neither failure can lose a handle: the global set already holds it.
        let _ = self.seen.try_with(|seen| {
            if let Ok(mut seen) = seen.try_borrow_mut() {
                seen.insert(raw);
            }
        });
    }
}

/// A slice length as the SDK's `int` element count, saturated at `i32::MAX`:
/// never negative and never more than the slice holds.
fn count(len: usize) -> i32 {
    i32::try_from(len).unwrap_or(i32::MAX)
}

/// Handles of one kind that this thread got from XPLM and has not given back,
/// each with how many times it holds it. X-Plane returns a live probe or
/// instance once, so their count is 1; an object is returned again by every
/// load of the same file and needs one unload per load.
///
/// The registries are per thread, like the XPLM calls themselves (module
/// note): a handle made on another thread is unknown here and its calls panic.
/// No borrow of a registry is held across an XPLM call.
#[derive(Default)]
struct Held(HashMap<usize, u32, BuildHasherDefault<AddressHash>>);

impl Held {
    fn add(&mut self, raw: usize) {
        *self.0.entry(raw).or_insert(0) += 1;
    }

    fn count(&self, raw: usize) -> u32 {
        self.0.get(&raw).copied().unwrap_or(0)
    }

    /// Gives one hold back; false when none was held.
    fn release(&mut self, raw: usize) -> bool {
        match self.0.get_mut(&raw) {
            Some(count) if *count > 1 => *count -= 1,
            Some(_) => {
                self.0.remove(&raw);
            }
            None => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::scenery::Objects;
    use super::{AddressSet, Found, Held, count};
    use std::cell::RefCell;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    thread_local! {
        static SEEN: RefCell<AddressSet> = RefCell::new(AddressSet::default());
    }

    #[test]
    fn found_handles_are_seen_from_every_thread() {
        // Only the registry is exercised; no SDK call is made.
        static FOUND: Found = Found::new(&SEEN);
        let handles: Vec<usize> = (1..=64).map(|i| 0x1000_0000 + i * 16).collect();
        for &handle in &handles {
            FOUND.insert(handle);
        }
        assert!(FOUND.contains(0));
        assert!(handles.iter().all(|&handle| FOUND.contains(handle)));
        assert!(!FOUND.contains(0x1000_0008));
        std::thread::spawn(move || {
            assert!(handles.iter().all(|&handle| FOUND.contains(handle)));
            assert!(!FOUND.contains(0x2000_0000));
        })
        .join()
        .unwrap();
    }

    #[test]
    fn found_ignores_the_null_handle() {
        thread_local! {
            static NULL_SEEN: RefCell<AddressSet> = RefCell::new(AddressSet::default());
        }
        static FOUND: Found = Found::new(&NULL_SEEN);
        FOUND.insert(0);
        assert!(FOUND.all.lock().unwrap().is_empty());
        assert!(NULL_SEEN.with_borrow(AddressSet::is_empty));
        assert!(FOUND.contains(0));
    }

    #[test]
    fn slice_counts_saturate_instead_of_going_negative() {
        assert_eq!(count(0), 0);
        assert_eq!(count(7), 7);
        assert_eq!(count(i32::MAX as usize), i32::MAX);
        assert_eq!(count(i32::MAX as usize + 1), i32::MAX);
        assert_eq!(count(usize::MAX), i32::MAX);
    }

    #[test]
    fn held_handles_count_each_hold_and_release_one_at_a_time() {
        let mut held = Held::default();
        assert_eq!(held.count(0x40), 0);
        assert!(!held.release(0x40));
        held.add(0x40);
        held.add(0x40);
        held.add(0x80);
        assert_eq!((held.count(0x40), held.count(0x80)), (2, 1));
        assert!(held.release(0x40));
        assert_eq!(held.count(0x40), 1);
        assert!(held.release(0x40));
        assert_eq!(held.count(0x40), 0);
        assert!(!held.release(0x40));
        assert_eq!(held.count(0x80), 1);
    }

    #[test]
    fn objects_need_one_unload_per_load_and_keep_their_last_load_for_instances() {
        // Only the registry is exercised; no SDK call is made.
        let mut objects = Objects::default();
        objects.loaded(0x100);
        objects.loaded(0x100);
        objects.instances.insert(0x900, 0x100);
        // A second load may go while an instance lives: the first still holds it.
        objects.release_load(0x100);
        assert_eq!(objects.objects.count(0x100), 1);
        let last = catch_unwind(AssertUnwindSafe(|| objects.release_load(0x100)));
        assert!(
            last.is_err(),
            "last unload with a live instance was allowed"
        );
        assert_eq!(objects.objects.count(0x100), 1);
        objects.instances.remove(&0x900);
        objects.release_load(0x100);
        assert_eq!(objects.objects.count(0x100), 0);
        let again = catch_unwind(AssertUnwindSafe(|| objects.release_load(0x100)));
        assert!(again.is_err(), "unload without a load was allowed");
    }
}
