# shtml

Server-side HTML rendering for Rust using a JSX-like macro syntax. The s is silent.

`shtml` is a `no_std` crate (using `alloc`) that lets you write HTML templates directly in Rust with the `view!` macro. It supports HTML elements, components, attributes, expressions, fragments, and automatic HTML escaping.

## Installation

```
cargo add --git https://github.com/swlkr/shtml shtml
```

## Quick start

```rust
use shtml::{view, Component, Elements, Render};

let page = view! {
    <!DOCTYPE html>
    <html lang="en">
        <head><title>My Page</title></head>
        <body><h1>Hello, world!</h1></body>
    </html>
};

assert_eq!(
    page.to_string(),
    r#"<!DOCTYPE html><html lang="en"><head><title>My Page</title></head><body><h1>Hello, world!</h1></body></html>"#
);
```

## Syntax reference

### HTML elements

Standard HTML elements with literal or dynamic attributes:

```rust
use shtml::{view, Component, Render};

// Literal attributes
let result = view! { <div class="container"><p>Hello</p></div> }.to_string();
assert_eq!(result, r#"<div class="container"><p>Hello</p></div>"#);

// Dynamic attributes
let class = "flex items-center h-full";
let result = view! { <div class=class></div> }.to_string();
assert_eq!(result, r#"<div class="flex items-center h-full"></div>"#);
```

### Void elements

Self-closing elements (`<br/>`, `<img/>`, `<input/>`, etc.) are handled automatically:

```rust
# use shtml::{view, Component, Render};
let result = view! { <input type="text" disabled/> }.to_string();
assert_eq!(result, r#"<input type="text" disabled/>"#);
```

### Boolean attributes

Attributes without a value are rendered as boolean attributes:

```rust
# use shtml::{view, Component, Render};
let result = view! { <input disabled/> }.to_string();
assert_eq!(result, "<input disabled/>");
```

### Spread attributes

Use `{..expr}` to spread a `Vec<(String, String)>` as attributes on elements or components:

```rust
# use shtml::{view, Component, Render};
let attrs = Vec::from([("data-id".to_string(), "42".to_string())]);
let result = view! { <div {..attrs}>content</div> }.to_string();
assert_eq!(result, r#"<div data-id="42">content</div>"#);
```

### Expressions

Embed Rust expressions with `{expr}`. The expression must implement `Render`:

```rust
# use shtml::{view, Component, Render};
let count = 42;
let result = view! { <span>{count}</span> }.to_string();
assert_eq!(result, "<span>42</span>");

let pi = 3.14;
let result = view! { <span>{pi}</span> }.to_string();
assert_eq!(result, "<span>3.14</span>");
```

### Components

Components are PascalCase functions that return `Component`. Attributes are passed as function arguments in declaration order. Children are passed as an `Elements` parameter:

```rust
#![allow(non_snake_case)]
use shtml::{view, Component, Elements, Render};

// Component with attributes
fn Greeting(name: &str) -> Component {
    view! { <p>Hello, {name}!</p> }
}

let result = view! { <Greeting name="world"/> }.to_string();
assert_eq!(result, "<p>Hello, world!</p>");

// Component with children
fn HStack(elements: Elements) -> Component {
    view! { <div class="flex gap-4">{elements}</div> }
}

let result = view! {
    <HStack>
        <div>1</div>
        <div>2</div>
        <div>3</div>
    </HStack>
}.to_string();
assert_eq!(result, r#"<div class="flex gap-4"><div>1</div><div>2</div><div>3</div></div>"#);

// Component with attributes and children
fn Heading(class: &str, elements: Elements) -> Component {
    view! { <h1 class=class>{elements}</h1> }
}

let result = view! {
    <Heading class="text-7xl text-red-500">
        <p>How now brown cow</p>
    </Heading>
}.to_string();
assert_eq!(result, r#"<h1 class="text-7xl text-red-500"><p>How now brown cow</p></h1>"#);
```

### Module-path components

Components can be referenced by their full module path:

```rust
#![allow(non_snake_case)]
use shtml::{view, Component, Elements, Render};

mod ui {
    use super::*;
    pub fn Card(elements: Elements) -> Component {
        view! { <div class="card">{elements}</div> }
    }
}

let result = view! { <ui::Card><p>Hello</p></ui::Card> }.to_string();
assert_eq!(result, r#"<div class="card"><p>Hello</p></div>"#);
```

### Fragments

Group elements without a wrapper using `<>...</>`:

```rust
# use shtml::{view, Component, Render};
let result = view! { <><div>A</div><div>B</div></> }.to_string();
assert_eq!(result, "<div>A</div><div>B</div>");
```

### Loops / iteration

Use `.iter().map(...).collect::<Vec<_>>()` inside an expression block:

```rust
#![allow(non_snake_case)]
use shtml::{view, Component, Elements, Render};

fn List(elements: Elements) -> Component {
    view! { <ul>{elements}</ul> }
}

fn Item(elements: Elements) -> Component {
    view! { <li>{elements}</li> }
}

let items = vec![1, 2, 3];
let result = view! {
    <List>
        {items.iter().map(|i| view! { <Item>{i}</Item> }).collect::<Vec<_>>()}
    </List>
}.to_string();
assert_eq!(result, "<ul><li>1</li><li>2</li><li>3</li></ul>");
```

### HTML escaping

String content (`&str`, `String`) is automatically HTML-escaped. `Component` values are not re-escaped since they contain already-rendered HTML:

```rust
# use shtml::{view, Component, Render};
let user_input = "<script>alert(\"xss\")</script>";
let result = view! { <div>{user_input}</div> }.to_string();
assert_eq!(result, r#"<div>&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;</div>"#);
```

The `escape()` function can also be used directly:

```rust
use shtml::escape;

assert_eq!(escape("<b>bold</b>"), "&lt;b&gt;bold&lt;/b&gt;");
assert_eq!(escape("no special chars"), "no special chars"); // zero-alloc
```

Characters escaped: `<` `>` `&` `"` `'`

## `Render` trait

The core abstraction for types that can be rendered inside `view!`. Any expression in `{...}` must implement `Render`.

### Built-in implementations

| Type | Behavior |
|------|----------|
| `&str`, `String` | HTML-escaped via `escape()` |
| `Component` | Appended as-is (already rendered) |
| Integer types (`u8`, `i8`, `u16`, `i16`, `i32`, `u32`, `i64`, `u64`, `usize`, `isize`) | Formatted via `itoa` (no allocation) |
| `f32`, `f64` | Formatted via `ryu` (no allocation) |
| `Vec<T: Render>` | Each element rendered sequentially |
| `Vec<(T, T)>` | Rendered as HTML attribute pairs (` key="value"`) |

### Custom implementations

```rust
use shtml::{view, Component, Render};

struct User { name: String }

impl Render for User {
    fn render_to_string(&self, buffer: &mut String) {
        buffer.push_str(&shtml::escape(&self.name));
    }
}

let user = User { name: "Alice".into() };
let result = view! { <span>{user}</span> }.to_string();
assert_eq!(result, "<span>Alice</span>");
```

## Feature flags

### `chaos`

The `chaos` feature enables the `#[component]` attribute macro, which transforms component functions into structs. This allows attributes to be passed in any order:

```rust,ignore
use shtml::{view, component, Component, Render};

#[component]
fn Chaos(a: &str, b: u8, c: String) -> Component {
    view! { <div a=a b=b c=c></div> }
}

// Attributes in any order:
let result = view! { <Chaos b=0 c="c".into() a="a"/> }.to_string();
assert_eq!(result, r#"<div a="a" b="0" c="c"></div>"#);
```

Without `chaos`, attributes must match the function parameter order:

```rust
# #![allow(non_snake_case)]
# use shtml::{view, Component, Render};
# fn Chaos(a: &str, b: u8, c: String) -> Component {
#     view! { <div a=a b=b c=c></div> }
# }
let result = view! { <Chaos a="a" b=0 c="c".into()/> }.to_string();
```

#### Optional props

With `#[component]`, a parameter of type `Option<T>` becomes an *optional prop*. It can
be omitted at the call site (defaulting to `None`) or supplied as `Some(value)`:

```rust,ignore
use shtml::{view, component, Component, Render};

#[component]
fn Badge(text: String, count: Option<u8>) -> Component {
    match count {
        Some(c) => view! { <span>{text}{c}</span> },
        None => view! { <span>{text}</span> },
    }
}

// Provided:
let result = view! { <Badge text="hi".into() count=Some(5)/> }.to_string();
assert_eq!(result, r#"<span>hi5</span>"#);

// Skipped (defaults to None):
let result = view! { <Badge text="hi".into()/> }.to_string();
assert_eq!(result, r#"<span>hi</span>"#);
```

Optional props also work alongside children. In `chaos` mode the children parameter must
be named `elements`:

```rust,ignore
use shtml::{view, component, Component, Elements, Render};

#[component]
fn Card(title: Option<String>, elements: Elements) -> Component {
    match title {
        Some(t) => view! { <div class="card"><h2>{t}</h2>{elements}</div> },
        None => view! { <div class="card">{elements}</div> },
    }
}

let result = view! { <Card><p>body</p></Card> }.to_string();
assert_eq!(result, r#"<div class="card"><p>body</p></div>"#);
```

Non-`Option` parameters remain required; omitting one panics at build time with
`missing required prop \`name\``. Note that `Option<&str>` is not supported as a prop
type — use `Option<String>` (a by-value `Option` holding a borrow would need a named
lifetime the macro does not infer).

### `context`

The `context` feature provides values once per render (auth session, CSRF token,
locale, ...) to every component, without passing them down as props. It requires `std`.

```toml
shtml = { features = ["chaos", "context"] }
```

Wrap the render in `provide`, and mark component params with `#[context]`:

```rust,ignore
use shtml::{view, component, context::provide, Component, Render};

#[component]
fn UserMenu(#[context] auth: AuthSession) -> Component {
    match &auth.user {
        Some(user) => view! { <span>{&user.name}</span> },
        None => view! { <a href="/login">Login</a> },
    }
}

#[component]
fn Page() -> Component {
    view! { <header><UserMenu/></header> }
}

// axum handler (with the `axum` feature, `Component` is a response)
async fn home(auth: AuthSession) -> Component {
    provide(auth, || view! { <Page/> })
}
```

- `#[context] auth: T` — required; panics at build time if no `T` was provided.
- `#[context] auth: Option<T>` — `None` if no `T` was provided.
- Passing the prop explicitly (`<UserMenu auth=other/>`) overrides the context.
- `T` must be owned and `Clone + 'static`; each lookup clones it (wrap expensive values in `Arc`).
- Nested `provide` calls with the same type shadow the outer value. Multiple types:
  `provide(auth, || provide(csrf, || ...))`.

Without `chaos`, read the context directly with `shtml::context::use_context::<T>()`
(returns `Option<T>`) or `expect_context::<T>()` (panics if missing).

The context is thread-local and exists only while the `provide` closure runs. Rendering is
synchronous, so this is safe in async handlers. There is no provider component: children
are rendered before their parent, so `provide` must wrap the whole render.

## Tips and tricks

- [leptosfmt](https://github.com/bram209/leptosfmt) with this override `rustfmt = { overrideCommand = ["leptosfmt", "--stdin", "--rustfmt", "--override-macro-names", "html"] }`
- [tree-sitter-rstml](https://github.com/rayliwell/tree-sitter-rstml) for html autocomplete inside of view! macros

For helix users: the view! macro should just work and have correct syntax highlighting and autocomplete with the default html lsp + tailwind if that's your jam

```toml
[language-server.tailwind-ls]
command = "tailwindcss-language-server"
args = ["--stdio"]

[language-server.tailwind-ls.config]
tailwindCSS = { experimental = { classRegex = ["class=\"(.*)\""] } }

[[language]]
name = "rust"
language-servers = ["rust-analyzer", "vscode-html-language-server", "tailwind-ls"]
```
