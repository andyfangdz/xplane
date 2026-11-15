use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};

use xplane_sdk_sys::{
    XPAddWidgetCallback, XPCreateCustomWidget, XPCreateWidget, XPDestroyWidget,
    XPGetWidgetGeometry, XPLMSetWindowPositioningMode, XPLMWindowID, XPWidgetFunc_t, XPWidgetID,
    XPWidgetPropertyID, xpMainWindowStyle_Translucent, xpProperty_MainWindowHasCloseBoxes,
    xpProperty_MainWindowType, xpWidgetClass_MainWindow,
};

use crate::{Bounds, WindowPosition, c_string};

/// An owned root widget: created by `XPCreateWidget`, destroyed once with its
/// children (`XPDestroyWidget(root, 1)`) on drop.
struct OwnedRoot(NonNull<c_void>);

impl OwnedRoot {
    fn live(&self) -> LiveWidget<'_> {
        LiveWidget(self.0, PhantomData)
    }
}

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        // SAFETY: this value created the root and destroys it, with its children, once.
        unsafe { XPDestroyWidget(self.0.as_ptr(), 1) };
    }
}

/// One of the widgets an [`OwnedRoot`] holds (the root or a child created
/// under it), live while the owner is borrowed: only the owner's drop
/// destroys them.
#[derive(Clone, Copy)]
struct LiveWidget<'a>(NonNull<c_void>, PhantomData<&'a OwnedRoot>);

/// Methods of [`LiveWidget`] for the class of widget calls that pass the
/// widget and plain numbers and nothing else: the widget is live for `'a`
/// (type note), and the other arguments are coordinates, property ids and
/// property values X-Plane only stores or compares as numbers. Members:
/// `XPSetWidgetProperty`, `XPShowWidget`, `XPBringRootWidgetToFront`,
/// `XPHideWidget`, `XPSetWidgetGeometry`, `XPGetWidgetUnderlyingWindow`.
macro_rules! widget_calls {
    ($(fn $name:ident(self $(, $arg:ident: $ty:ty)* $(,)?) $(-> $ret:ty)? = $sdk:ident;)+) => {$(
        fn $name(self $(, $arg: $ty)*) $(-> $ret)? {
            // SAFETY: a widget call (macro note): the widget is live for 'a; plain numbers otherwise.
            unsafe { xplane_sdk_sys::$sdk(self.0.as_ptr() $(, $arg)*) }
        }
    )+};
}

impl LiveWidget<'_> {
    widget_calls! {
        fn set_property(self, property: XPWidgetPropertyID, value: isize) = XPSetWidgetProperty;
        fn show(self) = XPShowWidget;
        fn bring_root_to_front(self) = XPBringRootWidgetToFront;
        fn hide(self) = XPHideWidget;
        fn set_geometry(self, left: i32, top: i32, right: i32, bottom: i32) = XPSetWidgetGeometry;
        fn underlying_window(self) -> XPLMWindowID = XPGetWidgetUnderlyingWindow;
    }

    fn geometry(self) -> Bounds {
        let mut bounds = Bounds::default();
        // SAFETY: the widget is live for 'a (type note); each out pointer is a distinct field of
        // `bounds`, live and exclusively borrowed for the call.
        unsafe {
            XPGetWidgetGeometry(
                self.0.as_ptr(),
                &mut bounds.left,
                &mut bounds.top,
                &mut bounds.right,
                &mut bounds.bottom,
            );
        }
        bounds
    }
}

/// An owned translucent root widget and its custom drawing child.
pub struct WidgetWindow {
    root: OwnedRoot,
    /// Created under `root`, so destroyed with it.
    content: NonNull<c_void>,
}

impl WidgetWindow {
    pub fn create_translucent(
        title: &str,
        bounds: Bounds,
        content_bounds: Bounds,
        callback: XPWidgetFunc_t,
    ) -> Result<Self, String> {
        if callback.is_none() {
            return Err("widget callback cannot be null".to_owned());
        }
        let title = c_string(title);
        let empty = c_string("");
        // SAFETY: descriptors remain live for the calls, the callback has the
        // widget ABI, and the root has no parent.
        let root = NonNull::new(unsafe {
            XPCreateWidget(
                bounds.left,
                bounds.top,
                bounds.right,
                bounds.bottom,
                0,
                title.as_ptr(),
                1,
                ptr::null_mut(),
                xpWidgetClass_MainWindow as i32,
            )
        })
        .map(OwnedRoot)
        .ok_or_else(|| "XPCreateWidget failed".to_owned())?;
        let live = root.live();
        live.set_property(
            xpProperty_MainWindowType,
            xpMainWindowStyle_Translucent as isize,
        );
        live.set_property(xpProperty_MainWindowHasCloseBoxes, 1);
        // SAFETY: `root` is live; the callback is non-null and has the widget ABI (residue, as
        // before: this safe function takes the raw callback its callers wrote for these widgets).
        unsafe { XPAddWidgetCallback(root.0.as_ptr(), callback) };
        // SAFETY: `root` is a live parent and the descriptor/callback remain
        // valid for the immediate creation call.
        let Some(content) = NonNull::new(unsafe {
            XPCreateCustomWidget(
                content_bounds.left,
                content_bounds.top,
                content_bounds.right,
                content_bounds.bottom,
                1,
                empty.as_ptr(),
                0,
                root.0.as_ptr(),
                callback,
            )
        }) else {
            // Dropping `root` recursively destroys the one live root widget.
            return Err("XPCreateCustomWidget failed".to_owned());
        };
        Ok(Self { root, content })
    }

    fn content(&self) -> LiveWidget<'_> {
        LiveWidget(self.content, PhantomData)
    }

    pub fn show(&self) {
        self.root.live().show();
        self.root.live().bring_root_to_front();
    }

    pub fn hide(&self) {
        self.root.live().hide();
    }

    pub fn root_geometry(&self) -> Bounds {
        self.root.live().geometry()
    }

    pub fn content_geometry(&self) -> Bounds {
        self.content().geometry()
    }

    pub fn set_geometry(&self, root: Bounds, content: Bounds) {
        self.root
            .live()
            .set_geometry(root.left, root.top, root.right, root.bottom);
        self.content()
            .set_geometry(content.left, content.top, content.right, content.bottom);
    }

    pub fn set_position(&self, position: WindowPosition) -> bool {
        let window = self.root.live().underlying_window();
        if window.is_null() {
            return false;
        }
        let mode = match position {
            WindowPosition::Free => xplane_sdk_sys::xplm_WindowPositionFree,
            WindowPosition::Vr => xplane_sdk_sys::xplm_WindowVR,
        };
        // SAFETY: XPLM returned this live underlying window for the owned root.
        unsafe { XPLMSetWindowPositioningMode(window, mode, -1) };
        true
    }

    pub fn is_root(&self, widget: XPWidgetID) -> bool {
        self.root.0.as_ptr() == widget
    }

    pub fn is_content(&self, widget: XPWidgetID) -> bool {
        self.content.as_ptr() == widget
    }
}
