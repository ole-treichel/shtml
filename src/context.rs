//! Scoped context: make values (auth session, locale, CSRF token, ...) available
//! to every component rendered inside a [`provide`] call, without passing them
//! down as props.
//!
//! Requires the `context` feature (which pulls in `std` for thread-local storage).
//!
//! # Usage
//!
//! With the `chaos` feature, mark a [`component`](crate::component) parameter with
//! `#[context]`. If the prop is not passed at the call site, it is looked up in the
//! context when the component is built:
//!
//! ```
//! # #![allow(non_snake_case)]
//! # #[cfg(feature = "chaos")]
//! # fn run() {
//! use shtml::{component, context::provide, view, Component, Render};
//!
//! #[derive(Clone)]
//! struct Auth { user: Option<String> }
//!
//! #[component]
//! fn UserMenu(#[context] auth: Auth) -> Component {
//!     match &auth.user {
//!         Some(user) => view! { <span>{user}</span> },
//!         None => view! { <a href="/login">Login</a> },
//!     }
//! }
//!
//! #[component]
//! fn Page() -> Component {
//!     view! { <header><UserMenu/></header> }
//! }
//!
//! let auth = Auth { user: Some("ole".into()) };
//! let page = provide(auth, || view! { <Page/> }.to_string());
//! assert_eq!(page, "<header><span>ole</span></header>");
//! # }
//! # #[cfg(not(feature = "chaos"))]
//! # fn run() {}
//! # run();
//! ```
//!
//! Without `chaos`, call [`use_context`] or [`expect_context`] directly:
//!
//! ```
//! use shtml::{context::{provide, use_context}, view, Component, Render};
//!
//! #[derive(Clone)]
//! struct Locale(&'static str);
//!
//! let page = provide(Locale("de"), || {
//!     let locale = use_context::<Locale>().unwrap();
//!     view! { <html lang=locale.0></html> }
//! });
//! assert_eq!(page.to_string(), r#"<html lang="de"></html>"#);
//! ```
//!
//! # Semantics
//!
//! - Values are keyed by type. Nested [`provide`] calls with the same type shadow the
//!   outer value until the inner call returns.
//! - Lookups clone the value (`C: Clone`). Wrap expensive values in `Arc`.
//! - The context is thread-local and only exists while the closure passed to
//!   [`provide`] runs. Rendering with [`view!`](crate::view) is synchronous, so this is
//!   safe inside async handlers (e.g. axum): no `.await` can happen during the render.
//!   A future returned from the closure runs outside the scope.
//! - There is no provider component: children are rendered before their parent, so a
//!   component cannot set context for its children. Call [`provide`] where the render
//!   starts, e.g. in the request handler.

extern crate std;

use std::{
    any::{type_name, Any},
    boxed::Box,
    cell::RefCell,
    vec::Vec,
};

std::thread_local! {
    static STACK: RefCell<Vec<Box<dyn Any>>> = const { RefCell::new(Vec::new()) };
}

/// Pops the value pushed by [`provide`] on drop, also when the closure panics.
struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        STACK.with(|stack| stack.borrow_mut().pop());
    }
}

/// Runs `f` with `value` available to [`use_context`], [`expect_context`] and
/// `#[context]` component props.
///
/// ```
/// use shtml::context::{provide, use_context};
///
/// let n = provide(42u32, || use_context::<u32>());
/// assert_eq!(n, Some(42));
/// assert_eq!(use_context::<u32>(), None);
/// ```
pub fn provide<C: 'static, R>(value: C, f: impl FnOnce() -> R) -> R {
    STACK.with(|stack| stack.borrow_mut().push(Box::new(value)));
    let _scope = Scope;
    f()
}

/// Returns a clone of the innermost provided value of type `C`, or `None` if no
/// enclosing [`provide`] call supplied one.
pub fn use_context<C: Clone + 'static>() -> Option<C> {
    STACK.with(|stack| {
        stack
            .borrow()
            .iter()
            .rev()
            .find_map(|value| value.downcast_ref::<C>())
            .cloned()
    })
}

/// Like [`use_context`], but panics if no value of type `C` was provided.
pub fn expect_context<C: Clone + 'static>() -> C {
    use_context().unwrap_or_else(|| {
        panic!(
            "no context of type `{}` provided; render inside `shtml::context::provide`",
            type_name::<C>()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic, string::String};

    #[test]
    fn it_is_empty_outside_provide() {
        assert_eq!(use_context::<u8>(), None);
    }

    #[test]
    fn it_provides_values_by_type() {
        provide(1u8, || {
            provide(String::from("a"), || {
                assert_eq!(use_context::<u8>(), Some(1));
                assert_eq!(use_context::<String>(), Some(String::from("a")));
                assert_eq!(use_context::<u16>(), None);
            })
        });
    }

    #[test]
    fn it_shadows_and_restores_outer_values() {
        provide(1u8, || {
            provide(2u8, || assert_eq!(use_context::<u8>(), Some(2)));
            assert_eq!(use_context::<u8>(), Some(1));
        });
        assert_eq!(use_context::<u8>(), None);
    }

    #[test]
    fn it_pops_on_panic() {
        let result = panic::catch_unwind(|| provide(1u8, || panic!("boom")));
        assert!(result.is_err());
        assert_eq!(use_context::<u8>(), None);
    }

    #[test]
    #[should_panic(expected = "no context of type `u8` provided")]
    fn it_panics_on_missing_expected_context() {
        expect_context::<u8>();
    }
}
