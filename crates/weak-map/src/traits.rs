use alloc::{rc, sync};

/// A trait for strong references.
pub trait StrongRef {
    /// The type of the weak reference.
    ///
    /// For example, for `std::rc::Rc<T>`, this will be `std::rc::Weak<T>`.
    type Weak: WeakRef<Strong = Self>;

    /// Constructs a weak reference from a strong reference.
    ///
    /// This is usually implemented by a `downgrade` method.
    fn downgrade(&self) -> Self::Weak;

    /// Compare two strong references for equality.
    ///
    /// This is usually implemented by a `ptr_eq` method.
    fn ptr_eq(&self, other: &Self) -> bool;
}

/// A trait for weak references.
pub trait WeakRef {
    /// The type of the strong reference.
    ///
    /// For example, for `std::rc::Weak<T>`, this will be `std::rc::Rc<T>`.
    type Strong: StrongRef<Weak = Self>;

    /// Acquires a strong reference from a weak reference.
    ///
    /// This is usually implemented by an `upgrade` method.
    fn upgrade(&self) -> Option<Self::Strong>;

    /// Is the given weak element expired?
    ///
    /// The default implemention checks whether a strong reference can be
    /// obtained via `upgrade`.
    fn is_expired(&self) -> bool {
        self.upgrade().is_none()
    }
}

impl<T: ?Sized> StrongRef for rc::Rc<T> {
    type Weak = rc::Weak<T>;

    fn downgrade(&self) -> Self::Weak {
        rc::Rc::downgrade(self)
    }

    fn ptr_eq(&self, other: &Self) -> bool {
        rc::Rc::ptr_eq(self, other)
    }
}

impl<T: ?Sized> WeakRef for rc::Weak<T> {
    type Strong = rc::Rc<T>;

    fn upgrade(&self) -> Option<Self::Strong> {
        self.upgrade()
    }

    fn is_expired(&self) -> bool {
        self.strong_count() == 0
    }
}

impl<T: ?Sized> StrongRef for sync::Arc<T> {
    type Weak = sync::Weak<T>;

    fn downgrade(&self) -> Self::Weak {
        sync::Arc::downgrade(self)
    }

    fn ptr_eq(&self, other: &Self) -> bool {
        sync::Arc::ptr_eq(self, other)
    }
}

impl<T: ?Sized> WeakRef for sync::Weak<T> {
    type Strong = sync::Arc<T>;

    fn upgrade(&self) -> Option<Self::Strong> {
        self.upgrade()
    }

    fn is_expired(&self) -> bool {
        self.strong_count() == 0
    }
}
