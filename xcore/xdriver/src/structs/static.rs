#[cfg(feature = "block")]
pub use crate::drivers::XBlockDevice;
#[cfg(feature = "display")]
pub use crate::drivers::XDisplayDevice;
#[cfg(feature = "net")]
pub use crate::drivers::XNetDevice;

impl super::XDeviceEnum {
    /// Constructs a network device.
    #[cfg(feature = "net")]
    pub const fn from_net(dev: XNetDevice) -> Self {
        Self::Net(dev)
    }

    /// Constructs a block device.
    #[cfg(feature = "block")]
    pub const fn from_block(dev: XBlockDevice) -> Self {
        Self::Block(dev)
    }

    /// Constructs a display device.
    #[cfg(feature = "display")]
    pub const fn from_display(dev: XDisplayDevice) -> Self {
        Self::Display(dev)
    }
}

/// A structure that contains all device drivers of a certain category.
///
/// If the feature `dyn` is enabled, the inner type is [`Vec<D>`]. Otherwise,
/// the inner type is [`Option<D>`] and at most one device can be contained.
pub struct XDeviceContainer<D>(Option<D>);

impl<D> XDeviceContainer<D> {
    /// Returns number of devices in this container.
    pub const fn len(&self) -> usize {
        if self.0.is_some() { 1 } else { 0 }
    }

    /// Returns whether the container is empty.
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Takes one device out of the container (will remove it from the container).
    pub fn take_one(&mut self) -> Option<D> {
        self.0.take()
    }

    /// Constructs the container from one device.
    pub const fn from_one(dev: D) -> Self {
        Self(Some(dev))
    }

    /// Adds one device into the container.
    #[allow(dead_code)]
    pub(crate) fn push(&mut self, dev: D) {
        if self.0.is_none() {
            self.0 = Some(dev);
        }
    }
}

impl<D> core::ops::Deref for XDeviceContainer<D> {
    type Target = Option<D>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<D> Default for XDeviceContainer<D> {
    fn default() -> Self {
        Self(Default::default())
    }
}
