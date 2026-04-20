//! Platform service registry — the trait-injection mechanism for plugin ops.
//!
//! Plugin ops that have been relocated to `dsl-runtime` but still need
//! capabilities that can't move (session state, DSL execution, stewardship,
//! etc.) consume those capabilities through trait objects registered by the
//! host platform at startup. The registry is the plumbing.
//!
//! # Shape
//!
//! - Host (ob-poc) constructs a [`ServiceRegistryBuilder`] at startup,
//!   registers each trait impl, and calls `build()`.
//! - The resulting [`ServiceRegistry`] is wrapped in `Arc` and attached to
//!   every [`crate::VerbExecutionContext`] the host dispatches.
//! - Plugin ops look up services via [`crate::VerbExecutionContext::service`],
//!   which wraps the registry miss with a friendly error that names the
//!   unregistered trait.
//!
//! # Registry keying
//!
//! The registry is keyed by [`std::any::TypeId`] of the trait object. That
//! makes registration and lookup perfectly type-safe — the compiler enforces
//! the binding — at the cost of requiring `'static` trait bounds (which is
//! fine; plugin op service traits are always `'static`).
//!
//! # Object safety
//!
//! Every trait passed to `register` must be object-safe (Rust checks this
//! at the call site when constructing the `Arc<dyn T>`). No generics on
//! trait methods, no `Self` in return positions except `Box<dyn T>`, no
//! `Self: Sized` bounds on methods.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Type-keyed registry of platform-provided service trait-objects.
///
/// Constructed once at startup via [`ServiceRegistryBuilder`] and held in
/// an `Arc` so it can be cheaply cloned onto every verb-execution context.
///
/// Lookups by trait `T` return `None` if the host did not register an impl
/// for that trait. Prefer [`crate::VerbExecutionContext::service`] in op
/// bodies — it wraps the miss with an actionable error message.
pub struct ServiceRegistry {
    services: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    /// An empty registry. Used by tests, by the default
    /// [`crate::VerbExecutionContext`], and by doc-examples that never
    /// resolve a service.
    pub fn empty() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Look up the impl registered under trait `T`.
    ///
    /// Returns `None` when no impl was registered. Lookups are O(1) HashMap
    /// operations over the trait `TypeId`.
    pub fn get<T: ?Sized + Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.services
            .get(&TypeId::of::<T>())?
            .downcast_ref::<Arc<T>>()
            .cloned()
    }

    /// True iff an impl is registered under trait `T`.
    pub fn has<T: ?Sized + Send + Sync + 'static>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<T>())
    }

    /// Number of registered services.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// True iff no services are registered.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistry")
            .field("count", &self.services.len())
            .finish()
    }
}

/// Builder for [`ServiceRegistry`].
///
/// Host code (ob-poc startup) registers each service trait impl here, then
/// freezes with [`Self::build`]. The resulting registry is `Send + Sync`
/// and can be cloned via `Arc`.
pub struct ServiceRegistryBuilder {
    services: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ServiceRegistryBuilder {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register an `Arc<dyn T>` under trait `T`.
    ///
    /// Panics on double-register of the same trait — the registry is a
    /// startup contract; duplicate registration is always a configuration
    /// bug, never valid.
    pub fn register<T: ?Sized + Send + Sync + 'static>(&mut self, service: Arc<T>) -> &mut Self {
        let key = TypeId::of::<T>();
        if self.services.contains_key(&key) {
            panic!(
                "Duplicate service registration for trait `{}` — check startup wiring",
                std::any::type_name::<T>()
            );
        }
        self.services.insert(key, Box::new(service));
        self
    }

    pub fn build(self) -> ServiceRegistry {
        ServiceRegistry {
            services: self.services,
        }
    }
}

impl Default for ServiceRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A fake trait for testing. Object-safe (no generics, no Self in
    // signatures except &self).
    trait Greeter: Send + Sync {
        fn greet(&self, name: &str) -> String;
    }

    struct Hello;
    impl Greeter for Hello {
        fn greet(&self, name: &str) -> String {
            format!("Hello, {}!", name)
        }
    }

    // A second, distinct trait to prove distinct TypeIds.
    trait Counter: Send + Sync {
        fn count(&self) -> usize;
    }

    struct StaticTwo;
    impl Counter for StaticTwo {
        fn count(&self) -> usize {
            2
        }
    }

    #[test]
    fn empty_registry_is_empty() {
        let r = ServiceRegistry::empty();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.get::<dyn Greeter>().is_none());
        assert!(!r.has::<dyn Greeter>());
    }

    #[test]
    fn register_and_get_roundtrip() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn Greeter>(Arc::new(Hello));
        let r = b.build();

        assert!(r.has::<dyn Greeter>());
        assert_eq!(r.len(), 1);

        let greeter = r.get::<dyn Greeter>().expect("trait registered");
        assert_eq!(greeter.greet("World"), "Hello, World!");
    }

    #[test]
    fn two_traits_coexist() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn Greeter>(Arc::new(Hello));
        b.register::<dyn Counter>(Arc::new(StaticTwo));
        let r = b.build();

        assert_eq!(r.len(), 2);
        assert_eq!(r.get::<dyn Greeter>().unwrap().greet("x"), "Hello, x!");
        assert_eq!(r.get::<dyn Counter>().unwrap().count(), 2);
    }

    #[test]
    fn miss_on_unregistered_trait_returns_none() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn Greeter>(Arc::new(Hello));
        let r = b.build();

        assert!(r.has::<dyn Greeter>());
        assert!(!r.has::<dyn Counter>());
        assert!(r.get::<dyn Counter>().is_none());
    }

    #[test]
    #[should_panic(expected = "Duplicate service registration for trait")]
    fn double_register_same_trait_panics() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn Greeter>(Arc::new(Hello));
        b.register::<dyn Greeter>(Arc::new(Hello));
    }

    #[test]
    fn arc_clone_preserves_registration() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn Greeter>(Arc::new(Hello));
        let r = Arc::new(b.build());

        let r2 = Arc::clone(&r);
        assert_eq!(r.get::<dyn Greeter>().unwrap().greet("a"), "Hello, a!");
        assert_eq!(r2.get::<dyn Greeter>().unwrap().greet("b"), "Hello, b!");
    }

    #[test]
    fn default_registry_is_empty() {
        let r = ServiceRegistry::default();
        assert!(r.is_empty());
        assert!(r.get::<dyn Greeter>().is_none());
    }

    #[test]
    fn debug_format_reports_count() {
        let mut b = ServiceRegistryBuilder::new();
        b.register::<dyn Greeter>(Arc::new(Hello));
        b.register::<dyn Counter>(Arc::new(StaticTwo));
        let r = b.build();

        let s = format!("{:?}", r);
        assert!(s.contains("count: 2"), "debug: {s}");
    }
}
