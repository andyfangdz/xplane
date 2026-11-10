//! Thread-bound plugin lifecycle with deferred cleanup during callback reentry.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::c_void,
    thread::{self, ThreadId},
};

/// Aircraft or plugin behavior invoked by [`Lifecycle`].
///
/// An instance handle identifies a resource from successful creation through
/// destruction, including time spent suspended. Keep handles distinct among all
/// live instances. Callbacks may synchronously reenter the lifecycle, so hosts
/// should release mutable resource borrows before calls that can reenter.
pub trait Host {
    type Instance: Copy + Ord;
    type Error;

    fn start(&self) -> Result<bool, Self::Error>;
    fn create(&self) -> Result<Self::Instance, Self::Error>;
    fn destroy(&self, instance: Self::Instance);
    fn reenabled(&self, instance: Self::Instance) -> Result<(), Self::Error>;
    fn stop(&self) -> Result<(), Self::Error>;

    /// Receives the SDK's message parameter for the duration of this callback.
    /// The lifecycle forwards the opaque value without dereferencing it.
    fn message(
        &self,
        instance: Self::Instance,
        from: i32,
        message: i32,
        parameter: *mut c_void,
    ) -> Result<(), Self::Error>;

    /// Indicates that a resource is in use outside lifecycle message dispatch.
    fn busy(&self, instance: Self::Instance) -> bool;
    /// Stops new callbacks on a resource that still has work in progress.
    fn suspend(&self, instance: Self::Instance);
    /// Requests a later call to [`Lifecycle::drain`] on the callback thread.
    fn defer(&self);
    fn report(&self, operation: &str, error: &Self::Error);
}

/// Owns the active instance handle and retires handles after callbacks return.
///
/// Lifecycle operations run on the thread that created this value. Calls from
/// another thread are ignored, returning `false` for start or enable.
pub struct Lifecycle<I> {
    owner: ThreadId,
    active: Cell<Option<I>>,
    enables: Cell<i32>,
    pending: RefCell<Vec<I>>,
    executing: RefCell<BTreeMap<I, u32>>,
}

struct Invocation<'a, I: Copy + Ord> {
    instance: I,
    executing: &'a RefCell<BTreeMap<I, u32>>,
}

impl<I: Copy + Ord> Drop for Invocation<'_, I> {
    fn drop(&mut self) {
        let mut executing = self.executing.borrow_mut();
        let count = executing.get_mut(&self.instance).unwrap();
        *count -= 1;
        if *count == 0 {
            executing.remove(&self.instance);
        }
    }
}

impl<I> Default for Lifecycle<I> {
    fn default() -> Self {
        Self {
            owner: thread::current().id(),
            active: Cell::new(None),
            enables: Cell::new(0),
            pending: RefCell::new(Vec::new()),
            executing: RefCell::new(BTreeMap::new()),
        }
    }
}

impl<I: Copy + Ord> Lifecycle<I> {
    fn on_owner_thread(&self) -> bool {
        self.owner == thread::current().id()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }

    pub fn current(&self) -> Option<I> {
        self.active.get()
    }

    pub fn start(&self, host: &impl Host<Instance = I>) -> bool {
        if !self.on_owner_thread() {
            return false;
        }
        self.enables.set(0);
        match host.start() {
            Ok(started) => started,
            Err(error) => {
                host.report("start", &error);
                let _ = host.stop();
                false
            }
        }
    }

    pub fn enable<H: Host<Instance = I>>(&self, host: &H) -> bool {
        if !self.on_owner_thread() {
            return false;
        }
        self.drain(host);
        let again = self.enables.get() > 0;
        self.enables.set(self.enables.get().wrapping_add(1));
        let instance = match host.create() {
            Ok(instance) => instance,
            Err(error) => {
                host.report("enable", &error);
                return false;
            }
        };
        if let Some(previous) = self.active.replace(Some(instance)) {
            self.release(host, previous);
        }
        if again && let Err(error) = self.invoke(host, instance, || host.reenabled(instance)) {
            host.report("enable", &error);
            return false;
        }
        true
    }

    fn busy(&self, host: &impl Host<Instance = I>, instance: I) -> bool {
        let executing = self.executing.borrow().contains_key(&instance);
        executing || host.busy(instance)
    }

    fn invoke<T, H: Host<Instance = I>>(
        &self,
        host: &H,
        instance: I,
        call: impl FnOnce() -> Result<T, H::Error>,
    ) -> Result<T, H::Error> {
        *self.executing.borrow_mut().entry(instance).or_default() += 1;
        let invocation = Invocation {
            instance,
            executing: &self.executing,
        };
        let result = call();
        drop(invocation);
        self.drain(host);
        result
    }

    fn release(&self, host: &impl Host<Instance = I>, instance: I) {
        if self.busy(host, instance) {
            host.suspend(instance);
            self.pending.borrow_mut().push(instance);
            host.defer();
        } else {
            host.destroy(instance);
        }
    }

    pub fn disable(&self, host: &impl Host<Instance = I>) {
        if !self.on_owner_thread() {
            return;
        }
        if let Some(instance) = self.active.take() {
            self.release(host, instance);
        }
        self.drain(host);
    }

    /// Stops the host after disabling it and retiring its instances.
    ///
    /// Aborts the process if an instance remains in use. Unloading the plugin
    /// in that state would invalidate an executing callback and its resources.
    pub fn stop(&self, host: &impl Host<Instance = I>) {
        if !self.on_owner_thread() {
            return;
        }
        self.disable(host);
        if self.has_pending() {
            std::process::abort();
        }
        if let Err(error) = host.stop() {
            host.report("stop", &error);
        }
    }

    pub fn receive<H: Host<Instance = I>>(
        &self,
        host: &H,
        from: i32,
        message: i32,
        parameter: *mut c_void,
    ) {
        if !self.on_owner_thread() {
            return;
        }
        if let Some(instance) = self.active.get()
            && let Err(error) = self.invoke(host, instance, || {
                host.message(instance, from, message, parameter)
            })
        {
            host.report("message", &error);
        }
    }

    /// Retires suspended instances whose callbacks have finished.
    pub fn drain(&self, host: &impl Host<Instance = I>) {
        if !self.on_owner_thread() {
            return;
        }
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        for instance in pending {
            if self.busy(host, instance) {
                self.pending.borrow_mut().push(instance);
            } else {
                host.destroy(instance);
            }
        }
    }
}

#[cfg(test)]
mod tests;
