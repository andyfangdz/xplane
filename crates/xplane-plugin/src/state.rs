use std::cell::RefCell;
use std::mem;

/// Storage for plugin state that remains explicitly bound to X-Plane's
/// callback thread instead of claiming that SDK or graphics handles are `Send`.
pub struct PluginStateSlot<T> {
    state: RefCell<Option<T>>,
}

impl<T> PluginStateSlot<T> {
    pub const fn new() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }

    pub fn with_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.state.borrow_mut().as_mut().map(callback)
    }

    pub fn replace(&self, state: Option<T>) -> Option<T> {
        mem::replace(&mut *self.state.borrow_mut(), state)
    }
}

impl<T> Default for PluginStateSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::PluginStateSlot;

    #[test]
    fn state_can_be_replaced_and_mutated() {
        let slot = PluginStateSlot::new();
        assert_eq!(slot.replace(Some(2)), None);
        assert_eq!(slot.with_mut(|value| *value += 3), Some(()));
        assert_eq!(slot.replace(None), Some(5));
    }
}
