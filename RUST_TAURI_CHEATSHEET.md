# Rust + Tauri 2 cheat sheet

A focused reference for the Rust and Tauri syntax that appears in this
`sys-dashboard` codebase. Every example is keyed to a real line you can
search for in your files, so you can read this side-by-side with the
source.

## Table of contents

1. [Rust basics](#1-rust-basics-that-appear-in-your-files)
2. [Macros and attributes](#2-the-macroattribute-syntax-youll-see)
3. [Tauri 2 vocabulary](#3-tauri-2-vocabulary-the-smallest-set-worth-memorising)
4. [Typed IPC contract](#4-your-typed-ipc-contract-handy-table)
5. [Idioms to add as you grow it](#5-idioms-to-add-to-the-codebase-as-you-grow-it)
6. [Compile-error decoder](#6-compile-error-decoder-youll-see-all-of-these)
7. [Useful one-liners](#7-useful-one-liners)

---

## 1. Rust basics that appear in your files

### Struct + `impl`

```rust
pub struct SystemMonitor {     // data
    inner: System,
}

impl SystemMonitor {           // methods on that data
    pub fn new() -> Self { ... }
    pub fn sample(&mut self) -> (f32, u64, u64) { ... }
}
```

- `Self` inside `impl` is shorthand for the type the `impl` block belongs to.
- `&self` = read-only borrow, `&mut self` = exclusive borrow, `self` (no `&`) = consumes the value.
- `pub` = visible outside the module. No `pub` = private to this file.

### Module system

Your tree maps directly:

```
src/lib.rs
  ├─ mod monitors;          → loads either monitors.rs or monitors/mod.rs
  └─ mod weather;
src/monitors/mod.rs
  └─ pub mod cpu_ram;       → loads cpu_ram.rs
```

- `mod foo;` declares a child module (loaded from a file).
- `use foo::Bar;` imports a name into the current scope.
- `crate::` refers to *your* crate's root, `super::` is parent module.

### Ownership cheats you'll meet

| Form | Meaning |
| --- | --- |
| `String` | owned, growable string on the heap |
| `&str` | borrowed view into a string |
| `"hello".to_string()` / `"hello".into()` | str → String |
| `T` | move/own |
| `&T` | shared borrow (many readers) |
| `&mut T` | exclusive borrow (one writer, no readers) |
| `Box<T>` | heap-allocated single owner |
| `Mutex<T>` | wrap T to allow safe mutation from multiple threads |

### `Result` and `Option` (used in every monitor)

```rust
fn sample(&mut self) -> Option<GpuStats> {
    let nvml = self.nvml.as_ref()?;             // ? on Option: short-circuit None
    let device = nvml.device_by_index(0).ok()?; // Result -> Option, then ?
    ...
    Some(GpuStats { ... })
}
```

- `Option<T>` = `Some(T)` or `None`.
- `Result<T, E>` = `Ok(T)` or `Err(E)`.
- `?` operator: in `Result`-returning fns it propagates errors; in `Option`-returning fns it propagates `None`.
- `.ok()` turns `Result<T, E>` into `Option<T>` (drops the error).
- `.map_err(|e| e.to_string())` rewrites the error type.

### Pattern matching

```rust
let nvml = match Nvml::init() {
    Ok(n) => Some(n),
    Err(e) => { eprintln!("..."); None }
};
```

Equivalent shorter forms you'll also see:

```rust
if let Some(stats) = gpu_stats { ... }       // run only on Some
let user = name.unwrap_or("anon".into());    // or default
```

### Closures

```rust
.map(|s| s.utilization_percent)    // |args| body
.map_err(|e| e.to_string())
```

### Trait objects (`dyn`)

```rust
pub weather: Box<dyn WeatherProvider>,
```

- `dyn Trait` = "any concrete type implementing this trait, decided at runtime".
- Needs heap storage (`Box<dyn ...>`) because the size isn't known statically.
- The trait must be **object-safe**: no generic methods, no `Self` returns. Yours is.

### `Send + Sync`

```rust
pub trait WeatherProvider: Send + Sync { ... }
```

- `Send` = can be moved across threads.
- `Sync` = `&T` can be shared across threads.
- Tauri's `manage()` requires both because state is shared across the IPC worker pool.

### `Mutex<T>` lock pattern

```rust
let mut sys = state.system.lock().map_err(|e| e.to_string())?;
//             ^^^^^^^^^^^   returns Result<MutexGuard<T>, PoisonError>
//                           A guard derefs to &mut T while in scope.
```

The lock is automatically released when `sys` goes out of scope (RAII).

---

## 2. The macro/attribute syntax you'll see

### `#[...]` is an attribute, `#![...]` applies to the whole file

```rust
#[derive(serde::Serialize)]            // auto-implement traits
#[serde(rename_all = "camelCase")]     // configure derive
struct SystemStats { ... }
```

- `#[derive(Trait, Trait)]` is the most common attribute - it asks the compiler/macro to generate a trait impl for you.
- The chained `#[serde(...)]` configures the derive that came before.

### `macro_name!(...)` - anything ending in `!` is a macro

```rust
vec![1, 2, 3]                            // build a Vec
println!("hello {}", name)              // formatted print
eprintln!("[gpu] init failed: {e}")     // print to stderr; {e} is shorthand for {} + e
tauri::generate_handler![cmd1, cmd2]    // builds an IPC dispatch table
tauri::generate_context!()              // bakes tauri.conf.json into the binary
```

Macros are not functions. They are expanded at compile time - they can take any token soup, declare new items, etc.

### `#[tauri::command]`

```rust
#[tauri::command]
fn get_system_stats(state: tauri::State<AppState>) -> Result<SystemStats, String> { ... }
```

Expands to a wrapper that:

1. Deserialises arguments from the JSON the frontend sent.
2. Calls your function.
3. Serialises the return value (or the `Err` string) back over IPC.

The function name is what the frontend uses: `invoke("get_system_stats")`.

---

## 3. Tauri 2 vocabulary (the smallest set worth memorising)

| Symbol | What it is |
| --- | --- |
| `tauri::Builder::default()` | Start configuring the app |
| `.manage(value)` | Stash a long-lived value. Retrieve later as `tauri::State<T>`. |
| `.invoke_handler(generate_handler![...])` | Register `#[tauri::command]` functions |
| `.run(generate_context!())` | Launch - blocks until the app exits |
| `tauri::State<T>` | Auto-injected reference to whatever you `.manage()`d |
| `tauri::AppHandle` | Send-able handle to the running app (events, windows) |
| `tauri::Window` | A specific window; lets you call `set_title`, `hide`, etc. |
| `tauri::generate_context!()` | Compile-time macro: embeds `tauri.conf.json` + icons |
| `tauri::generate_handler![a, b]` | Compile-time macro: builds the IPC dispatch table |

### The `invoke` round-trip

```
TS:    invoke<SystemStats>("get_system_stats", { foo: 1 })
                  │            │                  │
                  ▼            ▼                  ▼
          expected return  command name      args (object → JSON)

Rust:  #[tauri::command]
       fn get_system_stats(state: State<AppState>, foo: i32) -> Result<SystemStats, String>
```

Argument names on the Rust side become JSON keys on the JS side. Tauri serialises with `serde`, so `#[serde(rename_all = "camelCase")]` is what makes `temp_high_c` (Rust) become `tempHighC` (JS).

### Capabilities (Tauri 2 specific)

File: [src-tauri/capabilities/default.json](src-tauri/capabilities/default.json).

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- v2 replaced v1's `allowlist` with this granular capability system.
- `core:default` is a meta-set covering the basics: IPC, window dragging, events, etc.
- If you ever invoke a built-in plugin (e.g. `notification`, `fs`), add `"notification:default"` here.

### The two-file `main.rs` / `lib.rs` split (your repo)

```rust
// main.rs
fn main() { sys_dashboard_lib::run(); }

// lib.rs
pub fn run() {
    tauri::Builder::default().run(tauri::generate_context!())
}
```

Why: mobile targets compile only the library. Keeping logic in `lib.rs` means one code path everywhere, plus integration tests can `use sys_dashboard_lib::*;`.

---

## 4. Your typed IPC contract (handy table)

| Frontend (TS, [src/api.ts](src/api.ts)) | Backend (Rust, [src-tauri/src/lib.rs](src-tauri/src/lib.rs) / [weather/mod.rs](src-tauri/src/weather/mod.rs)) |
| --- | --- |
| `getSystemStats(): Promise<SystemStats>` | `#[tauri::command] fn get_system_stats(state: State<AppState>) -> Result<SystemStats, String>` |
| `getForecast(): Promise<Forecast>` | `#[tauri::command] fn get_forecast(state: State<AppState>) -> Result<Forecast, String>` |
| `interface SystemStats { cpuPercent: number; ... }` | `#[derive(Serialize)] #[serde(rename_all="camelCase")] struct SystemStats { cpu_percent: f32, ... }` |
| `null` field (optional) | `Option<T>` field |
| `Promise.reject("...")` | `Err("...".to_string())` |

Mental model: serde + Tauri = shape-matched JSON over an in-process channel.

---

## 5. Idioms to add to the codebase as you grow it

```rust
// async command (Tauri auto-runs it on the worker pool)
#[tauri::command]
async fn fetch_thing() -> Result<Thing, String> { ... }

// emit an event from Rust to ALL frontend listeners
app.emit("stats-updated", payload).unwrap();
```

```ts
// receive an event in TS:
import { listen } from "@tauri-apps/api/event";
listen<Stats>("stats-updated", (e) => console.log(e.payload));
```

```rust
// open a second window
tauri::WebviewWindowBuilder::new(&app, "settings",
    tauri::WebviewUrl::App("settings.html".into()))
    .title("Settings").build()?;

// share state behind Arc when commands run concurrently and lock is too coarse
use std::sync::Arc;
state.client.clone()  // Arc::clone is cheap (refcount ++)
```

---

## 6. Compile-error decoder (you'll see all of these)

| Error | Mental fix |
| --- | --- |
| `cannot move out of borrowed content` | Use `.clone()`, or change `&self` to `self`, or use `Arc<T>` |
| `borrowed value does not live long enough` | Bind to a `let` so it lives as long as you use the borrow |
| `the trait X is not implemented for Y` | Add `#[derive(X)]`, or write `impl X for Y` |
| `cannot find macro X! in this scope` | `use crate::X;` or check the import path |
| `expected struct Foo, found &Foo` | `&` mismatch - drop the `&` or add one |
| `value used after move` | The variable was consumed; re-bind earlier or borrow with `&` |
| `the size for values of type dyn Trait cannot be known at compilation time` | Wrap in `Box<dyn Trait>` or `&dyn Trait` |

---

## 7. Useful one-liners

```bash
cargo check                # type-check without producing a binary (fast)
cargo clippy               # lints (worth running periodically)
cargo fmt                  # canonical formatting
cargo doc --open           # build + open the docs for ALL deps
cargo tree                 # see what pulls in what
cargo expand               # see what macros expand to (cargo install cargo-expand)
```

`cargo expand` is great for demystifying `#[tauri::command]` and `#[derive(Serialize)]` - run it once on your `lib.rs` and you'll see exactly what the macros generate.
