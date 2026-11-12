use crate::guard::*;
use libloading::Library;
use serde_json::Value;
use std::{
    ffi::{CStr, CString, c_char},
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
};
use xphost_core::{Backend, Config, Request};

type Call = unsafe extern "C" fn(*const c_char) -> *const c_char;
type Attach = unsafe extern "C" fn(Option<Message>);
type Start = unsafe extern "C" fn(*mut c_char, *mut c_char, *mut c_char) -> i32;
type Enable = unsafe extern "C" fn() -> i32;
type Void = unsafe extern "C" fn();

/// One plugin and its SDK, bound to the creating simulation thread. Libraries
/// outlive every copied symbol and registration; shutdown always precedes unload.
pub struct Host {
    plugin: Option<Library>,
    _fake: Library,
    call: Call,
    attach: Attach,
    disable: Option<Void>,
    stop: Option<Void>,
    message: Option<Message>,
    started: bool,
    enabled: bool,
    faulted: bool,
    _gl: Option<GlContext>,
    _thread: PhantomData<Rc<()>>,
}
fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: callers use exact SDK/control C ABI signatures, and retain the
    // owning library longer than every copied function pointer.
    unsafe {
        library
            .get::<T>(name)
            .map(|s| *s)
            .map_err(|e| e.to_string())
    }
}
fn seh(code: u32) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("plugin fault (SEH 0x{code:08x})"))
    }
}
impl Host {
    pub fn load(config: Config, fake_path: &Path, navdat: Option<&Path>) -> Result<Self, String> {
        // SAFETY: loading native code is the purpose of this host. Keep libraries
        // alive until all lifecycle callbacks and SDK registrations are gone.
        let fake =
            unsafe { Library::new(fake_path) }.map_err(|e| format!("cannot load fake SDK: {e}"))?;
        let call = symbol::<Call>(&fake, b"XpHost_Call\0")?;
        let version = symbol::<unsafe extern "C" fn() -> u32>(&fake, b"XpHost_ProtocolVersion\0")?;
        // SAFETY: no-argument private version export belongs to the loaded SDK.
        if unsafe { version() } != xphost_core::PROTOCOL_VERSION {
            return Err("incompatible xphost SDK protocol".into());
        }
        let attach = symbol::<Attach>(&fake, b"XpHost_Attach\0")?;
        let mut host = Self {
            plugin: None,
            _fake: fake,
            call,
            attach,
            disable: None,
            stop: None,
            message: None,
            started: false,
            enabled: false,
            faulted: false,
            _gl: GlContext::new(),
            _thread: PhantomData,
        };
        println!(
            "xphost: GL context {}",
            if host._gl.is_some() {
                "ready (hidden window)"
            } else {
                "unavailable (continuing without GL)"
            }
        );
        host.request(Request::Init(Box::new(config.clone())))?;
        if let Some(path) = navdat {
            host.request(Request::Nav(path.to_string_lossy().into_owned()))?;
        }
        host.request(Request::Log(format!(
            "host loading {} for {}",
            config.plugin_path, config.acf_path
        )))?;
        // SAFETY: plugin remains owned by Host, preloaded fake SDK resolves its
        // XPLM imports. The caller explicitly selected this native plugin.
        let plugin = unsafe { Library::new(PathBuf::from(&config.plugin_path)) }
            .map_err(|e| format!("cannot load win.xpl: {e}"))?;
        let start = symbol::<Start>(&plugin, b"XPluginStart\0")?;
        let enable = symbol::<Enable>(&plugin, b"XPluginEnable\0")?;
        host.disable = Some(symbol::<Void>(&plugin, b"XPluginDisable\0")?);
        host.stop = Some(symbol::<Void>(&plugin, b"XPluginStop\0")?);
        host.message = Some(symbol::<Message>(&plugin, b"XPluginReceiveMessage\0")?);
        host.plugin = Some(plugin);
        let (mut name, mut signature, mut description) = ([0i8; 256], [0i8; 256], [0i8; 256]);
        let mut result = 0;
        // SAFETY: SDK start receives three writable 256-byte buffers. C traps
        // native faults immediately around the foreign call.
        seh(unsafe {
            xp_start(
                start,
                name.as_mut_ptr(),
                signature.as_mut_ptr(),
                description.as_mut_ptr(),
                &mut result,
            )
        })?;
        name[255] = 0;
        signature[255] = 0;
        description[255] = 0;
        let text = |s: &[i8]| {
            // SAFETY: fixed buffers have just been NUL-terminated.
            unsafe { CStr::from_ptr(s.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        };
        println!(
            "xphost: XPluginStart -> {result} ({} / {})",
            text(&name),
            text(&signature)
        );
        if result == 0 {
            return Err("plugin rejected XPluginStart".into());
        }
        host.started = true;
        host.request(Request::Info {
            name: text(&name),
            signature: text(&signature),
            description: text(&description),
        })?;
        // SAFETY: plugin library owns this entry point and is still loaded.
        seh(unsafe { xp_enable(enable, &mut result) })?;
        println!("xphost: XPluginEnable -> {result}");
        if result == 0 {
            return Err("plugin rejected XPluginEnable".into());
        }
        host.enabled = true;
        host.request(Request::Enabled(true))?;
        // SAFETY: retain attached entry point until shutdown detaches it.
        unsafe {
            (host.attach)(host.message);
        }
        Ok(host)
    }
    pub fn send_message(&mut self, id: i32) {
        if let Some(cb) = self.message {
            // SAFETY: message entry point belongs to the still-loaded plugin.
            let code = unsafe { xp_message(cb, 0, id, std::ptr::null_mut()) };
            if let Err(error) = seh(code) {
                self.faulted = true;
                println!("xphost: {error}");
                let _ = self.request(Request::Log(format!("XPluginReceiveMessage: {error}")));
            }
        }
    }
    pub fn shutdown(&mut self) -> bool {
        if self.enabled {
            self.enabled = false;
            if let Some(cb) = self.disable {
                // SAFETY: callback is loaded; flags are cleared before entering it.
                let code = unsafe { xp_void(cb) };
                if let Err(error) = seh(code) {
                    self.faulted = true;
                    let _ = self.request(Request::Log(format!("XPluginDisable: {error}")));
                }
            }
            let _ = self.request(Request::Enabled(false));
        }
        if self.started {
            self.started = false;
            if let Some(cb) = self.stop {
                // SAFETY: stop precedes plugin unload, including failed enables.
                let code = unsafe { xp_void(cb) };
                if let Err(error) = seh(code) {
                    self.faulted = true;
                    let _ = self.request(Request::Log(format!("XPluginStop: {error}")));
                }
            }
        }
        // SAFETY: remove callback before dropping its owning plugin library.
        unsafe {
            (self.attach)(None);
        }
        self.faulted
    }
}
impl Backend for Host {
    fn request(&self, request: Request) -> Result<Value, String> {
        let input = CString::new(serde_json::to_string(&request).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        // SAFETY: private control export receives a live CString. It returns a
        // NUL-terminated thread-local response valid until the next call. Host
        // cannot be sent/shared between threads, and copies before another call.
        let response = unsafe { (self.call)(input.as_ptr()) };
        if response.is_null() {
            return Err("empty SDK response".into());
        }
        // SAFETY: non-null response follows the private protocol contract above.
        let bytes = unsafe { CStr::from_ptr(response) }.to_bytes();
        let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if let Some(error) = value.get("error") {
            return Err(error.as_str().unwrap_or("SDK error").into());
        }
        value
            .get("ok")
            .cloned()
            .ok_or_else(|| "invalid SDK response".into())
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.shutdown();
        self.plugin.take();
    }
}

use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Gdi::*, OpenGL::*},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
struct GlContext {
    window: HWND,
    dc: HDC,
    context: HGLRC,
}
impl GlContext {
    fn new() -> Option<Self> {
        // SAFETY: Win32 structs are initialized, hidden window/DC/context are
        // owned by this RAII value and used only on their creating thread.
        unsafe {
            let class: Vec<u16> = "xphost-rust-gl\0".encode_utf16().collect();
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                64,
                64,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if window.is_null() {
                return None;
            }
            let mut value = Self {
                window,
                dc: GetDC(window),
                context: std::ptr::null_mut(),
            };
            if value.dc.is_null() {
                return None;
            }
            let pfd = PIXELFORMATDESCRIPTOR {
                nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
                nVersion: 1,
                dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL,
                iPixelType: PFD_TYPE_RGBA,
                cColorBits: 24,
                cDepthBits: 16,
                ..std::mem::zeroed()
            };
            let format = ChoosePixelFormat(value.dc, &pfd);
            if format == 0 || SetPixelFormat(value.dc, format, &pfd) == 0 {
                return None;
            }
            value.context = wglCreateContext(value.dc);
            if value.context.is_null() || wglMakeCurrent(value.dc, value.context) == 0 {
                return None;
            }
            Some(value)
        }
    }
}
impl Drop for GlContext {
    fn drop(&mut self) {
        // SAFETY: release only resources owned by this context, on the same thread.
        unsafe {
            if !self.context.is_null() {
                wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
                wglDeleteContext(self.context);
            }
            if !self.dc.is_null() {
                ReleaseDC(self.window, self.dc);
            }
            DestroyWindow(self.window);
        }
    }
}
