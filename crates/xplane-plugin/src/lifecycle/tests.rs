use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Instance(u32);

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Created(Instance),
    Destroyed(Instance),
    Reenabled(Instance),
    Suspended(Instance),
    Deferred,
    Reported,
    Received(i32),
    Returned(i32),
}

#[derive(Default)]
struct Fixture {
    lifecycle: Lifecycle<Instance>,
    next_id: Cell<u32>,
    fail_create: Cell<bool>,
    trace: RefCell<Vec<Event>>,
}

impl Fixture {
    fn record(&self, event: Event) {
        self.trace.borrow_mut().push(event);
    }
}

impl Host for Fixture {
    type Instance = Instance;
    type Error = &'static str;

    fn start(&self) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn create(&self) -> Result<Instance, Self::Error> {
        if self.fail_create.get() {
            return Err("create failed");
        }
        let id = self.next_id.replace(self.next_id.get() + 1);
        let instance = Instance(id);
        self.record(Event::Created(instance));
        Ok(instance)
    }

    fn destroy(&self, instance: Instance) {
        self.record(Event::Destroyed(instance));
    }

    fn reenabled(&self, instance: Instance) -> Result<(), Self::Error> {
        self.record(Event::Reenabled(instance));
        Ok(())
    }

    fn stop(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn message(
        &self,
        _: Instance,
        _: i32,
        message: i32,
        _: *mut c_void,
    ) -> Result<(), Self::Error> {
        self.record(Event::Received(message));
        match message {
            1 => self.lifecycle.receive(self, 0, 2, ptr::null_mut()),
            2 => self.lifecycle.disable(self),
            3 => {
                self.lifecycle.disable(self);
                panic!("injected callback unwind");
            }
            _ => unreachable!(),
        }
        assert!(self.lifecycle.has_pending());
        self.record(Event::Returned(message));
        Ok(())
    }

    fn busy(&self, _: Instance) -> bool {
        false
    }

    fn suspend(&self, instance: Instance) {
        assert_eq!(self.lifecycle.current(), None);
        self.record(Event::Suspended(instance));
    }

    fn defer(&self) {
        self.record(Event::Deferred);
    }

    fn report(&self, _: &str, _: &Self::Error) {
        self.record(Event::Reported);
    }
}

#[test]
fn nested_disable_retires_instance_after_outer_message_returns() {
    let host = Fixture::default();
    let lifecycle = &host.lifecycle;
    assert!(lifecycle.start(&host));
    assert!(lifecycle.enable(&host));
    lifecycle.receive(&host, 0, 1, ptr::null_mut());

    assert_eq!(lifecycle.current(), None);
    assert!(!lifecycle.has_pending());
    assert_eq!(
        *host.trace.borrow(),
        [
            Event::Created(Instance(0)),
            Event::Received(1),
            Event::Received(2),
            Event::Suspended(Instance(0)),
            Event::Deferred,
            Event::Returned(2),
            Event::Returned(1),
            Event::Destroyed(Instance(0)),
        ]
    );
}

#[test]
fn failed_creation_keeps_active_instance_until_replacement_succeeds() {
    let host = Fixture::default();
    let lifecycle = &host.lifecycle;
    assert!(lifecycle.enable(&host));
    host.fail_create.set(true);
    assert!(!lifecycle.enable(&host));
    assert_eq!(lifecycle.current(), Some(Instance(0)));

    host.fail_create.set(false);
    assert!(lifecycle.enable(&host));
    assert_eq!(lifecycle.current(), Some(Instance(1)));
    assert_eq!(
        *host.trace.borrow(),
        [
            Event::Created(Instance(0)),
            Event::Reported,
            Event::Created(Instance(1)),
            Event::Destroyed(Instance(0)),
            Event::Reenabled(Instance(1)),
        ]
    );
}

#[test]
fn unwind_releases_invocation_count_without_running_cleanup() {
    let host = Fixture::default();
    let lifecycle = &host.lifecycle;
    assert!(lifecycle.enable(&host));
    let result = catch_unwind(AssertUnwindSafe(|| {
        lifecycle.receive(&host, 0, 3, ptr::null_mut())
    }));
    assert!(result.is_err());
    assert!(lifecycle.has_pending());
    assert_eq!(host.trace.borrow().last(), Some(&Event::Deferred));

    lifecycle.drain(&host);
    assert!(!lifecycle.has_pending());
    assert_eq!(
        host.trace.borrow().last(),
        Some(&Event::Destroyed(Instance(0)))
    );
}
