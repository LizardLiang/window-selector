# Learn Rust Through window-selector

A hands-on Rust lesson built entirely from YOUR codebase. Every example is real code you wrote.

---

## Table of Contents

1. [Project Structure & Module System](#1-project-structure--module-system)
2. [Ownership, Borrowing & Lifetimes](#2-ownership-borrowing--lifetimes)
3. [Structs & Methods](#3-structs--methods)
4. [Enums & Pattern Matching](#4-enums--pattern-matching)
5. [Traits & Polymorphism](#5-traits--polymorphism)
6. [Error Handling](#6-error-handling)
7. [Collections & Iterators](#7-collections--iterators)
8. [Closures](#8-closures)
9. [Generics & Type Parameters](#9-generics--type-parameters)
10. [unsafe Rust & FFI](#10-unsafe-rust--ffi)
11. [Smart Pointers & Memory Layout](#11-smart-pointers--memory-layout)
12. [Concurrency Primitives (Single-Thread Usage)](#12-concurrency-primitives-single-thread-usage)
13. [Testing](#13-testing)
14. [Design Patterns in This Codebase](#14-design-patterns-in-this-codebase)
15. [Cargo & Dependencies](#15-cargo--dependencies)
16. [Reading Order: Where to Start](#16-reading-order-where-to-start)

---

## 1. Project Structure & Module System

### How Rust organizes code

Rust uses a **module tree** rooted at `main.rs` (for binaries) or `lib.rs` (for libraries).
Every `.rs` file in `src/` is a module, but it only becomes part of your program when you
declare it with `mod`.

**Your `main.rs` (lines 3-24):**
```rust
mod about_dialog;
mod accent_color;
mod animation;
mod config;
mod dwm_thumbnails;
mod grid_layout;
mod hotkey;
// ... 15 more modules
```

Each `mod foo;` tells the compiler: "load `src/foo.rs` and make it a child module of `main`."
Without this declaration, the file is **ignored** — even if it exists on disk.

### Visibility: `pub` vs private

By default, everything in Rust is **private**. To use something from another module, mark it `pub`.

**`state.rs:8-23` — public enum, public fields:**
```rust
pub enum OverlayState {    // pub → other modules can use this type
    Hidden,
    FadingIn,
    Active {
        selected: Option<usize>,  // fields inside enum variants are always public
    },
    FadingOut {
        switch_target: Option<HWND>,
    },
}
```

**`animation.rs:31-34` — public struct, public fields:**
```rust
pub struct FadeAnimator {
    pub current_alpha: u8,        // pub → overlay.rs can read this
    pub direction: Option<FadeDirection>,
}
```

### `use` imports

`use` brings items into scope so you don't write the full path every time.

**`interaction.rs:1-8`:**
```rust
use crate::state::{OverlayState, SessionTags};  // crate:: = root of this project
use crate::window_info::WindowInfo;
use windows::Win32::Foundation::HWND;            // external crate
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_RETURN,     // import multiple items from one path
    VK_SPACE, VK_ESCAPE, VK_A, VK_Z,
};
```

**Key syntax:**
- `crate::` — start from your project root
- `super::` — go up one module (parent)
- `self::` — current module (rarely needed)
- `use X as Y` — rename on import

### `pub(crate)` — visible within the crate but not outside

You'll sometimes see `pub(crate)` which means "public to other modules in this crate, but
not to external users." Since this is a binary (not a library), `pub` and `pub(crate)` are
functionally identical here.

---

## 2. Ownership, Borrowing & Lifetimes

This is Rust's core innovation. Every value has exactly ONE owner. When the owner goes out
of scope, the value is dropped (freed).

### Move semantics

**`main.rs:207-217`:**
```rust
let mut app_state = Box::new(AppState { ... });
//  ^^^^^^^^^^^^^^^^ app_state OWNS this heap allocation

let app_state_ptr = app_state.as_mut() as *mut AppState;
//                  ^^^^^^^^^ borrow, then cast to raw pointer — ownership stays with app_state

// ... later at line 292:
drop(app_state);  // explicitly drop — frees the heap memory
```

If you tried `let x = app_state;` somewhere in between, `app_state` would be **moved** into
`x`, and using `app_state` afterward would be a compile error.

### Borrowing: `&` and `&mut`

- `&T` — shared/immutable reference. Multiple allowed simultaneously.
- `&mut T` — exclusive/mutable reference. Only ONE at a time.

**`interaction.rs:62-68`:**
```rust
pub fn handle_key_down(
    vk_code: u32,
    state: &OverlayState,          // shared borrow — just reading
    windows: &[WindowInfo],         // shared borrow of a slice
    tags: &mut SessionTags,         // exclusive borrow — we might modify
    direct_switch: bool,            // copy — u32/bool implement Copy
) -> KeyAction {
```

Why `&[WindowInfo]` instead of `&Vec<WindowInfo>`? A **slice** (`&[T]`) is more flexible — it
works with any contiguous data, not just `Vec`. This is a common Rust idiom.

### The borrow checker in action

This won't compile:
```rust
let mut v = vec![1, 2, 3];
let first = &v[0];     // shared borrow
v.push(4);             // ERROR: can't mutate while shared borrow exists
println!("{}", first); // first is still alive here
```

In your codebase, you work around this naturally:

**`main.rs:595-596`:**
```rust
let snap = app.window_snapshot.clone();  // clone to get owned copy
app.overlay_manager.show(&snap, &mut app.overlay_state);
// Now app.overlay_manager borrows snap (not app.window_snapshot)
// so there's no conflict with &mut app.overlay_state
```

### Clone vs Copy

- **`Copy`** — bitwise copy, automatic. For simple types: `u8`, `i32`, `f32`, `bool`, `char`.
- **`Clone`** — explicit deep copy via `.clone()`. For heap types: `String`, `Vec<T>`.

**`window_info.rs:4` — `#[derive(Clone)]` means you can call `.clone()` on WindowInfo:**
```rust
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,      // String is on the heap → needs Clone, not Copy
    pub is_minimized: bool,  // bool is Copy
    // ...
}
```

### Lifetimes (brief)

Lifetimes appear when the compiler can't figure out how long a reference is valid.
Your codebase avoids explicit lifetimes by using owned types and raw pointers for the
Win32 interop. You'll mostly encounter lifetimes when writing generic functions that
accept references.

---

## 3. Structs & Methods

### Defining structs

**`grid_layout.rs:5-13`:**
```rust
#[derive(Debug, Clone, Copy, PartialEq)]  // derive = auto-implement traits
pub struct CellRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub window_index: usize,
}
```

**Derive macros explained:**
- `Debug` — enables `{:?}` formatting for printing
- `Clone` — enables `.clone()`
- `Copy` — enables implicit bitwise copy (only for simple stack types)
- `PartialEq` — enables `==` comparison

### `impl` blocks: methods and associated functions

**`grid_layout.rs:15-32`:**
```rust
impl CellRect {
    // This is a METHOD — takes &self as first parameter
    pub fn scaled(&self, factor: f32) -> CellRect {
        let new_w = self.width * factor;
        let new_h = self.height * factor;
        let cx = self.x + self.width / 2.0;
        let cy = self.y + self.height / 2.0;
        CellRect {
            x: cx - new_w / 2.0,
            y: cy - new_h / 2.0,
            width: new_w,
            height: new_h,
            window_index: self.window_index,
        }
    }
}
```

**`state.rs:51-56` — associated function (no `self` parameter, like a "static method"):**
```rust
impl SessionTags {
    pub fn new() -> Self {   // Self = SessionTags. This is a constructor.
        Self {
            tags: HashMap::new(),
        }
    }
}
```

Call it: `SessionTags::new()` — note the `::` syntax, not `.`.

### `&self` vs `&mut self` vs `self`

```rust
pub fn get(&self, number: u8) -> Option<HWND>    // read-only access
pub fn assign(&mut self, number: u8, hwnd: HWND) // modifying access
pub fn into_inner(self) -> HashMap<u8, HWND>      // consumes the struct (takes ownership)
```

---

## 4. Enums & Pattern Matching

Rust enums are **algebraic data types** — each variant can hold different data.

### Defining enums

**`state.rs:8-23`:**
```rust
pub enum OverlayState {
    Hidden,                           // no data
    FadingIn,                         // no data
    Active { selected: Option<usize> }, // struct-like variant with named field
    FadingOut { switch_target: Option<HWND> }, // struct-like variant
}
```

**`interaction.rs:44-56`:**
```rust
pub enum KeyAction {
    None,
    Select(usize),        // tuple-like variant holding a usize
    SwitchTo(HWND),       // tuple-like variant holding an HWND
    Dismiss,
    TagAssigned,
}
```

### `match` — exhaustive pattern matching

`match` forces you to handle EVERY variant. The compiler errors if you miss one.

**`main.rs:308-342` — the main message dispatcher:**
```rust
match msg {
    WM_CREATE => LRESULT(0),

    WM_DESTROY => {
        PostQuitMessage(0);
        LRESULT(0)
    }

    WM_HOTKEY => {
        if wparam.0 as i32 == hotkey::HOTKEY_ID {
            handle_hotkey(app);
        }
        LRESULT(0)
    }

    // ... more arms ...

    _ => DefWindowProcW(hwnd, msg, wparam, lparam),  // _ = catch-all
}
```

### Destructuring in match

**`main.rs:568-574`:**
```rust
match &app.overlay_state {
    OverlayState::Hidden => activate_overlay(app),
    OverlayState::Active { .. } => {     // { .. } = "I don't care about the fields"
        dismiss_overlay(app);
    }
    _ => {}                               // FadingIn, FadingOut → do nothing
}
```

**`main.rs:676-698` — matching KeyAction with destructuring:**
```rust
match action {
    KeyAction::None => {}
    KeyAction::Select(idx) => {           // extract the usize from Select
        app.overlay_state = OverlayState::Active { selected: Some(idx) };
        // ...
    }
    KeyAction::SwitchTo(target) => {      // extract the HWND from SwitchTo
        app.overlay_manager.begin_hide(&mut app.overlay_state, Some(target));
    }
    KeyAction::Dismiss => {
        dismiss_overlay(app);
    }
    KeyAction::TagAssigned => { /* ... */ }
}
```

### `if let` — match a single pattern

When you only care about ONE variant:

**`state.rs:37-42`:**
```rust
pub fn selected_index(&self) -> Option<usize> {
    if let OverlayState::Active { selected } = self {
        *selected       // dereference to get Option<usize>
    } else {
        None
    }
}
```

### `matches!` macro — boolean pattern check

**`state.rs:27-29`:**
```rust
pub fn is_visible(&self) -> bool {
    !matches!(self, OverlayState::Hidden)  // true for everything except Hidden
}
```

---

## 5. Traits & Polymorphism

Traits are Rust's version of interfaces — they define shared behavior.

### `impl Trait` for your types

**`state.rs:95-99` — implementing `Default` trait:**
```rust
impl Default for SessionTags {
    fn default() -> Self {
        Self::new()
    }
}
```

Now `SessionTags::default()` works, and any generic code that requires `T: Default` accepts
`SessionTags`.

### `Drop` — custom destructor

**`dwm_thumbnails.rs:23-31`:**
```rust
impl Drop for ThumbnailHandle {
    fn drop(&mut self) {
        if self.thumbnail_id != 0 {
            unsafe {
                let _ = DwmUnregisterThumbnail(self.thumbnail_id);
            }
        }
    }
}
```

When a `ThumbnailHandle` goes out of scope, Rust automatically calls `drop()`. This is
**RAII** (Resource Acquisition Is Initialization) — resources are cleaned up deterministically.

**`mru_tracker.rs:109-113` — MruTracker also uses Drop:**
```rust
impl Drop for MruTracker {
    fn drop(&mut self) {
        self.uninstall_hook();  // automatically unhook when tracker is destroyed
    }
}
```

**`window_switcher.rs:21-37` — RAII guard for thread input:**
```rust
struct ThreadInputGuard {
    our_thread: u32,
    target_thread: u32,
}

impl Drop for ThreadInputGuard {
    fn drop(&mut self) {
        // Always detach thread input, even on panic or early return
        let ok = unsafe {
            AttachThreadInput(self.our_thread, self.target_thread, false).as_bool()
        };
        // ...
    }
}
```

### Derive macros = auto-implementing traits

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
```

This is syntactic sugar. The compiler generates the trait implementations for you based on
the struct's fields. Only works for traits that have a standard derivation path.

### `Serialize` / `Deserialize` — external derive macros

**`config.rs:7-8`:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig { ... }
```

These come from the `serde` crate (line 7 in Cargo.toml). They auto-generate code to
convert AppConfig to/from TOML, JSON, etc.

---

## 6. Error Handling

Rust has no exceptions. Errors are values.

### `Result<T, E>` — recoverable errors

**`config.rs:34`:**
```rust
pub fn load(config_dir: &Path) -> std::io::Result<Self> {
    // std::io::Result<Self> is shorthand for Result<Self, std::io::Error>
```

### The `?` operator — propagate errors

**`config.rs:64-78`:**
```rust
fn save_to_path(config: &AppConfig, config_path: &Path) -> std::io::Result<()> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;   // ? = if this errors, return the error immediately
    }

    let toml_str = toml::to_string(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    //  ^^^^^^^^ convert toml::ser::Error into std::io::Error, then ?

    let tmp_path = config_path.with_extension("toml.tmp");
    fs::write(&tmp_path, &toml_str)?;  // ? again
    fs::rename(&tmp_path, config_path)?;

    Ok(())  // success! return the "empty" Ok
}
```

### `Option<T>` — nullable values without null

**`window_info.rs:11-12`:**
```rust
pub letter: Option<char>,       // Some('a') or None
pub number_tag: Option<u8>,     // Some(1) or None
```

**Using Option:**
```rust
// Pattern match
if let Some(letter) = win.letter {
    // use letter
}

// Unwrap with default
let tag = win.number_tag.unwrap_or(0);

// Map/transform
let upper: Option<String> = win.letter.map(|c| c.to_uppercase().to_string());
```

### Fallback patterns with `unwrap_or_else`

**`main.rs:123-124`:**
```rust
let config_dir = AppConfig::default_config_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("./config"));
// If None, compute a fallback lazily
```

**`main.rs:134-140`:**
```rust
let config = match AppConfig::load(&config_dir) {
    Ok(c) => c,
    Err(e) => {
        tracing::error!("Config load failed: {}", e);
        AppConfig::default()    // fall back to defaults on error
    }
};
```

### `let _ = ...` — explicitly ignore errors

Throughout the codebase you'll see:
```rust
let _ = SetForegroundWindow(hwnd);
```

The `let _ =` tells Rust "I know this returns a Result/value, and I'm intentionally ignoring it."
Without it, the compiler warns about unused `Result`.

---

## 7. Collections & Iterators

### `Vec<T>` — growable array

**Creating:**
```rust
let mut monitors: Vec<MonitorInfo> = Vec::new();  // empty
let windows: Vec<WindowInfo> = vec![];             // vec! macro, also empty
let mut buf = vec![0u16; (title_len as usize) + 1]; // vec of zeros
```

**Common operations:**
```rust
monitors.push(info);                    // append
monitors.sort_by_key(|m| if m.is_primary { 0 } else { 1 });  // sort
ctx.windows.retain(|w| !own_hwnds.contains(&w.hwnd));  // filter in-place
self.order.insert(0, hwnd);            // insert at index
self.order.truncate(MRU_MAX_SIZE);     // cap length
self.thumbnails.clear();                // remove all
```

### `HashMap<K, V>` — key-value store

**`state.rs:47-49`:**
```rust
pub struct SessionTags {
    tags: HashMap<u8, HWND>,   // number key → window handle
}
```

**Operations:**
```rust
self.tags.insert(number, hwnd);            // insert/overwrite
self.tags.get(&number).copied()            // lookup → Option<HWND>
self.tags.retain(|_, hwnd| unsafe { IsWindow(*hwnd).as_bool() });  // filter in-place
```

### Iterator chains

Iterators are Rust's killer feature for data transformation. They're **lazy** — nothing
happens until you "consume" the chain (with `.collect()`, `.for_each()`, etc.).

**`mru_tracker.rs:87-92` — build a HashMap from an iterator:**
```rust
let mru_pos: HashMap<isize, usize> = self
    .order
    .iter()                          // iterate over Vec<HWND>
    .enumerate()                     // (index, &hwnd)
    .map(|(i, &h)| (h.0 as isize, i))  // transform to (isize, usize) pairs
    .collect();                      // collect into HashMap
```

**`grid_layout.rs:79-93` — generate cells with map + collect:**
```rust
let cells = (0..window_count)        // range 0, 1, 2, ... window_count-1
    .map(|i| {                       // transform each index into a CellRect
        let row = i / cols;
        let col = i % cols;
        let x = PADDING + col as f32 * (cell_width + PADDING);
        let y = PADDING + row as f32 * (cell_height + PADDING);
        CellRect { x, y, width: cell_width, height: cell_height, window_index: i }
    })
    .collect();                      // collect into Vec<CellRect>
```

**`state.rs:69-74` — find with iterators:**
```rust
pub fn get_tag_for_hwnd(&self, hwnd: HWND) -> Option<u8> {
    self.tags
        .iter()                       // iterate (&u8, &HWND) pairs
        .find(|(_, &h)| h == hwnd)    // find first match
        .map(|(&n, _)| n)            // extract just the number
}
```

**`letter_assignment.rs:24-26` — position (indexOf):**
```rust
pub fn find_by_letter(windows: &[WindowInfo], letter: char) -> Option<usize> {
    windows.iter().position(|w| w.letter == Some(letter))
}
```

**Common iterator methods cheat sheet:**
| Method | What it does |
|--------|-------------|
| `.iter()` | iterate by reference (`&T`) |
| `.iter_mut()` | iterate by mutable reference (`&mut T`) |
| `.into_iter()` | iterate by value (consumes the collection) |
| `.map(fn)` | transform each element |
| `.filter(fn)` | keep elements where fn returns true |
| `.find(fn)` | first element where fn returns true |
| `.position(fn)` | index of first match |
| `.enumerate()` | add index: `(usize, T)` |
| `.collect()` | consume iterator, build a collection |
| `.count()` | count elements |
| `.any(fn)` | true if any element matches |
| `.take(n)` | first n elements |

---

## 8. Closures

Closures are anonymous functions that can capture variables from their environment.

**Syntax:** `|params| body` or `|params| { multi-line body }`

**`mru_tracker.rs:94-99`:**
```rust
windows.sort_by_key(|w| {           // w is each &WindowInfo
    mru_pos
        .get(&(w.hwnd.0 as isize))  // captures mru_pos from outer scope
        .copied()
        .unwrap_or(usize::MAX)
});
```

**`window_enumerator.rs:54-61`:**
```rust
ctx.windows.retain(|w| {            // retain takes a closure
    !own_hwnds.contains(&w.hwnd)    // captures own_hwnds from outer scope
});
```

**Closures vs function pointers:**
```rust
// Function pointer — no captured variables
pub type KeyHandler = fn(vk_code: u32) -> bool;

// Closure — can capture environment
let threshold = 100;
let filter = |x: &i32| *x > threshold;  // captures `threshold`
```

---

## 9. Generics & Type Parameters

### In the standard library

You use generics constantly without realizing it:
```rust
Vec<WindowInfo>           // Vec is generic over T
HashMap<u8, HWND>         // HashMap is generic over K, V
Option<usize>             // Option is generic over T
Result<(), std::io::Error> // Result is generic over T, E
```

### In your code

**`hotkey.rs:41` — generic-like pattern with `Vec<&str>`:**
```rust
pub fn format_hotkey(modifiers: u32, vk_code: u32) -> String {
    let mut parts = Vec::new();       // Rust infers Vec<&str> from usage below
    if (modifiers & 0x0002) != 0 {
        parts.push("Ctrl");           // &str pushed → Vec<&str>
    }
    parts.join("+")                   // joins into String
}
```

### `as` — type casting

Numeric conversions are explicit in Rust (no implicit widening/narrowing):
```rust
let n = window_count as f32;     // usize → f32
let cmd = (wparam.0 & 0xFFFF) as u32;  // usize → u32
let c = (b'a' + (vk - VK_A.0 as u32) as u8) as char;  // chain of casts
```

---

## 10. `unsafe` Rust & FFI

Your codebase is heavily `unsafe` because it calls raw Win32 APIs.

### What `unsafe` means

`unsafe` does NOT mean "bad code." It means: "I, the programmer, am manually guaranteeing
invariants that the compiler can't verify."

`unsafe` unlocks five abilities:
1. Dereference raw pointers (`*const T`, `*mut T`)
2. Call `unsafe` functions
3. Access mutable statics
4. Implement `unsafe` traits
5. Access fields of `union` types

### Raw pointers

**`main.rs:88-90`:**
```rust
fn get_app_state() -> *mut AppState {
    APP_STATE_PTR.load(std::sync::atomic::Ordering::Relaxed) as *mut AppState
}
```

`*mut AppState` is a raw pointer — like C's `AppState*`. It has no borrow checker protection.
You must manually ensure:
- The pointer is valid (not null, not dangling)
- No aliasing violations (not two `&mut` at the same time)

**Dereferencing a raw pointer (line 306):**
```rust
unsafe {
    let app = &mut *app_ptr;  // raw pointer → mutable reference
}
```

### `unsafe extern "system" fn` — Win32 callbacks

**`main.rs:296-301`:**
```rust
unsafe extern "system" fn main_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
```

- `unsafe` — this function may be called in unsafe contexts
- `extern "system"` — use the Windows calling convention (stdcall on x86)
- Windows calls this function directly — you must match its exact signature

### Callback pattern with LPARAM

Win32 callbacks pass context via an integer (`LPARAM`). Rust pattern:

**`window_enumerator.rs:39-51`:**
```rust
let ctx_ptr = &mut ctx as *mut EnumContext;  // Rust struct → raw pointer

unsafe {
    let _ = EnumWindows(
        Some(enum_windows_callback),  // function pointer to our callback
        LPARAM(ctx_ptr as isize),     // context packed as integer
    );
}

// In the callback:
unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumContext);  // unpack integer → pointer → reference
    // now use ctx.windows, ctx.monitors, etc.
}
```

### `std::mem::transmute` — reinterpret cast

**`keyboard_hook.rs:115`:**
```rust
let handler: KeyHandler = std::mem::transmute(handler_ptr);
```

`transmute` reinterprets raw bytes as a different type. Extremely dangerous — use only when
you're absolutely sure the types have compatible layouts (here: `usize` → function pointer).

### `std::mem::forget` — prevent Drop from running

**`logging.rs:58`:**
```rust
std::mem::forget(_guard);
```

The tracing guard must live for the entire process. Normally it would be dropped at function
end, closing the log file. `forget` leaks it intentionally.

---

## 11. Smart Pointers & Memory Layout

### `Box<T>` — heap allocation

**`main.rs:207`:**
```rust
let mut app_state = Box::new(AppState { ... });
```

`Box::new()` allocates `AppState` on the **heap** (not the stack). This matters because:
1. `AppState` is too large for comfortable stack usage
2. We need a **stable pointer** — stack addresses change; heap addresses don't

### Stack vs Heap

```
Stack (fast, automatic)          Heap (slower, manual/Box)
┌────────────────────┐           ┌────────────────────┐
│ vk_code: u32       │           │ AppState {         │
│ result: KeyAction  │           │   config: ...      │
│ idx: usize         │           │   overlay_state    │
└────────────────────┘           │   window_snapshot  │
  (freed when function returns)  │   ...              │
                                 └────────────────────┘
                                   (freed when Box is dropped)
```

### `Vec<T>` internals

`Vec` stores a pointer, length, and capacity on the stack. The actual data lives on the heap:

```
Stack: Vec<WindowInfo>         Heap:
┌──────────────┐               ┌──────────────────┐
│ ptr ──────────────────────── → │ WindowInfo[0]    │
│ len: 5       │               │ WindowInfo[1]    │
│ capacity: 8  │               │ WindowInfo[2]    │
└──────────────┘               │ WindowInfo[3]    │
                               │ WindowInfo[4]    │
                               │ (unused) [5..7]  │
                               └──────────────────┘
```

---

## 12. Concurrency Primitives (Single-Thread Usage)

Your codebase uses concurrency primitives in a single-threaded context. This is unusual
but intentional — it avoids `static mut` (which is unsound in Rust 2024).

### `AtomicUsize` — thread-safe integer

**`main.rs:83-84`:**
```rust
static APP_STATE_PTR: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
```

Why not just `static mut ptr: usize = 0`?
- `static mut` requires `unsafe` on every access and is considered unsound
- `AtomicUsize` is `Send + Sync` (safe to put in a `static`)
- The atomic operations are overkill for single-thread, but they're correct AND safe

### `AtomicBool`

**`keyboard_hook.rs:21`:**
```rust
static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_active(active: bool) {
    HOOK_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn is_active() -> bool {
    HOOK_ACTIVE.load(Ordering::Relaxed)
}
```

`Ordering::Relaxed` means "no synchronization guarantees beyond atomicity." This is fine for
single-threaded code — there's no other thread to synchronize with.

### `OnceLock` — write-once global

**`mru_tracker.rs:129`:**
```rust
static MRU_TRACKER_CELL: std::sync::OnceLock<SendPtr> = std::sync::OnceLock::new();
```

`OnceLock` can be set exactly once. After that, reads are lock-free. Used for globals that
are initialized at startup and never change.

### `unsafe impl Send + Sync`

**`mru_tracker.rs:122-125`:**
```rust
struct SendPtr(*mut MruTracker);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}
```

Raw pointers (`*mut T`) are neither `Send` nor `Sync` by default because the compiler can't
verify they're used safely across threads. Here, the programmer manually asserts: "I guarantee
this pointer is only accessed on one thread."

---

## 13. Testing

### Test structure

Tests live in a `#[cfg(test)]` module at the bottom of each file:

**`grid_layout.rs:134-136`:**
```rust
#[cfg(test)]    // only compile this module during `cargo test`
mod tests {
    use super::*;  // import everything from the parent module
```

### Writing tests

**`animation.rs:103-117`:**
```rust
#[test]    // marks this function as a test
fn test_fade_in_reaches_max() {
    let mut anim = FadeAnimator::new();
    anim.start_fade_in();
    assert!(anim.is_animating());      // panic if false

    let mut ticks = 0;
    while anim.tick() {
        ticks += 1;
        assert!(ticks < 20, "Fade-in should complete within 20 ticks");
        // custom message on failure ↑
    }
    assert_eq!(anim.current_alpha, ALPHA_MAX);  // assert equality
    assert!(!anim.is_animating());
}
```

### Test helper functions

**`state.rs:105-107`:**
```rust
fn hwnd(n: isize) -> HWND {
    HWND(n as *mut _)   // create fake HWNDs for testing
}
```

**`interaction.rs:181-185`:**
```rust
fn make_window_info(hwnd_n: isize, letter: char) -> WindowInfo {
    let mut w = WindowInfo::new(hwnd(hwnd_n), format!("Window {}", hwnd_n), false, 0);
    w.letter = Some(letter);
    w
}
```

### Testing pure logic vs Win32 code

The codebase is designed so that **pure logic is testable**:
- `grid_layout.rs` — zero Win32 dependency, 8 tests
- `animation.rs` — zero Win32 dependency, 5 tests
- `interaction.rs` — minimal Win32 (only `GetAsyncKeyState`), 15+ tests
- `letter_assignment.rs` — zero Win32 dependency, 6 tests

While **Win32-coupled code** is harder to test:
- `overlay.rs` — requires real HWNDs, tested manually
- `window_switcher.rs` — requires real windows, tested manually

### Run tests

```bash
cargo test                          # all tests
cargo test config::tests            # only config module tests
cargo test -- --test-threads=1      # if tests share Win32 state
cargo test -- --nocapture           # show println! output
```

---

## 14. Design Patterns in This Codebase

### Pattern 1: State Machine

The core of the app is a state machine:

```
         ┌──────────────────────────────────┐
         │                                  │
         ▼                                  │
      Hidden ──hotkey──→ FadingIn ──done──→ Active
         ▲                   │               │
         │                   │ (dismiss)     │ (escape / select+enter)
         │                   ▼               ▼
         └───────────── FadingOut ◄──────────┘
                           │
                           │ done + target?
                           ▼
                    switch_to_window()
```

**`state.rs`** defines the enum. **`main.rs`** checks it before every action:

```rust
match &app.overlay_state {
    OverlayState::Hidden => activate_overlay(app),
    OverlayState::Active { .. } => dismiss_overlay(app),
    _ => {} // FadingIn/FadingOut — ignore
}
```

### Pattern 2: RAII (Resource Acquisition Is Initialization)

Resources are tied to object lifetimes. When the object is dropped, the resource is released.

| Resource | Type | Cleanup in `Drop` |
|----------|------|-------------------|
| DWM thumbnail | `ThumbnailHandle` | `DwmUnregisterThumbnail` |
| WinEvent hook | `MruTracker` | `UnhookWinEvent` |
| Thread attachment | `ThreadInputGuard` | `AttachThreadInput(false)` |

### Pattern 3: Callback Context via Raw Pointer

Win32 callbacks don't support closures. The pattern is:
1. Create a context struct
2. Cast its pointer to `LPARAM` (integer)
3. In the callback, cast `LPARAM` back to a pointer

Used in: `window_enumerator.rs`, `monitor.rs`, `mru_tracker.rs`.

### Pattern 4: Global State via Atomic

Instead of `static mut` (unsound), the codebase uses atomics:

```rust
static APP_STATE_PTR: AtomicUsize = AtomicUsize::new(0);  // main.rs
static OVERLAY_WNDPROC_PTR: AtomicUsize = ...;             // overlay.rs
static HOOK_HANDLE: AtomicUsize = ...;                      // keyboard_hook.rs
static HOOK_ACTIVE: AtomicBool = ...;                       // keyboard_hook.rs
static MRU_TRACKER_CELL: OnceLock<SendPtr> = ...;          // mru_tracker.rs
```

The safety invariant is always the same: **only the message pump thread accesses these.**

### Pattern 5: Command Pattern

`handle_key_down()` returns a `KeyAction` **enum** instead of performing the action directly.
This separates **decision** from **execution**:

```
Input (vk_code) → handle_key_down() → KeyAction::Select(3)
                                            ↓
                                    main.rs match block → actually updates state
```

Benefits:
- `handle_key_down()` is **testable** (returns a value, doesn't mutate global state)
- The caller (main.rs) decides HOW to execute (can add logging, animation, etc.)

### Pattern 6: Snapshot Pattern

At overlay activation, the entire window list is **snapshotted** into `Vec<WindowInfo>`:

```rust
app.window_snapshot = snapshot_windows(...);
```

All subsequent operations work on this snapshot — not live Win32 state. This prevents:
- Windows appearing/disappearing mid-interaction
- Race conditions with other apps
- Inconsistent state between grid layout and key handling

### Pattern 7: Layered Windows (Two-HWND Rendering)

```
Z-order (top to bottom):
┌─────────────────────────────┐
│ Label Overlay (GDI)         │  ← letter badges, number tags
│  WS_EX_TRANSPARENT          │    click-through, color-keyed
├─────────────────────────────┤
│ DWM Thumbnails              │  ← live window previews
│  (composited by Windows)     │    rendered above the D2D surface
├─────────────────────────────┤
│ Primary Overlay (Direct2D)   │  ← backdrop, cell backgrounds, aura glow
│  WS_EX_LAYERED              │    alpha-blended
└─────────────────────────────┘
```

DWM thumbnails are composited between the two HWNDs. Labels must be on a separate window
because DWM renders thumbnails ABOVE the host window's surface.

---

## 15. Cargo & Dependencies

### Cargo.toml

```toml
[package]
name = "window-selector"       # crate name (becomes window_selector in code)
version = "0.1.0"
edition = "2021"                # Rust edition (affects syntax/features available)

[dependencies]
serde = { version = "1", features = ["derive"] }   # serialization framework
toml = "0.8"                                        # TOML parser (for config)
tracing = "0.1"                                     # structured logging
tracing-appender = "0.2"                            # log file rotation
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dependencies.windows]          # Microsoft's official Win32 bindings
version = "0.58"
features = [                    # only pull in the APIs you actually use
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Dwm",       # DWM thumbnail APIs
    "Win32_Graphics_Direct2D",  # 2D rendering
    # ... more features
]

[profile.release]
opt-level = 3        # maximum optimization
lto = true           # link-time optimization (slower build, faster binary)
codegen-units = 1    # compile as single unit (better optimization)
```

### Key commands

```bash
cargo build                  # debug build → target/debug/
cargo build --release        # release build → target/release/
cargo test                   # compile + run all #[test] functions
cargo clippy                 # linter — catches common mistakes
cargo doc --open             # generate + open API docs in browser
cargo run                    # build + run
cargo run -- --debug         # run with CLI args
```

---

## 16. Reading Order: Where to Start

Read the codebase in this order, from simplest to most complex:

### Tier 1: Pure Rust, no Win32 (start here)

| File | Concepts | Lines |
|------|----------|-------|
| `window_info.rs` | Structs, Option, derive macros | 26 |
| `letter_assignment.rs` | Arrays, iterators, tests | 106 |
| `animation.rs` | Enums, state mutation, saturating math | 163 |
| `grid_layout.rs` | Functions, f32 math, iterators, tests | 269 |
| `accent_color.rs` | Small struct, methods, one unsafe call | 73 |

### Tier 2: Logic with light Win32 dependency

| File | Concepts | Lines |
|------|----------|-------|
| `state.rs` | Enums with data, HashMap, pattern matching, tests | 176 |
| `interaction.rs` | Command pattern, match, Option chaining, tests | 477 |
| `config.rs` | Serde, file I/O, Result, ? operator, atomic writes | 165 |
| `hotkey.rs` | Small Win32 wrappers, string formatting | 109 |
| `logging.rs` | Trait composition, builder pattern, mem::forget | 84 |

### Tier 3: Win32 integration

| File | Concepts | Lines |
|------|----------|-------|
| `monitor.rs` | Callback pattern, pointer casting | 95 |
| `mru_tracker.rs` | MRU data structure, WinEvent hook, OnceLock, Drop | 201 |
| `window_enumerator.rs` | EnumWindows callback, filtering, snapshot | 233 |
| `window_switcher.rs` | RAII guard, focus transfer strategies | 112 |
| `keyboard_hook.rs` | Low-level hook, AtomicBool, function pointers | 127 |
| `tray.rs` | System tray, popup menus, UTF-16 encoding | 151 |

### Tier 4: Rendering & orchestration

| File | Concepts | Lines |
|------|----------|-------|
| `dwm_thumbnails.rs` | DWM API, aspect ratio math, Drop for cleanup | 238 |
| `overlay.rs` | Window creation, Z-order management, animation | 457 |
| `overlay_renderer.rs` | Direct2D, DirectWrite, GPU resource management | 661 |
| `main.rs` | Entry point, message loop, wndproc, all orchestration | 730 |

---

## Quick Reference: Rust Syntax Cheat Sheet

```rust
// Variables
let x = 5;              // immutable
let mut y = 10;          // mutable
let z: u32 = 42;        // explicit type

// Functions
fn add(a: i32, b: i32) -> i32 { a + b }  // last expression = return value

// String types
let s: &str = "hello";        // string slice (borrowed, immutable)
let s: String = "hello".to_string();  // owned String (heap allocated)
let s: String = format!("hello {}", name);  // formatted

// If/else (is an expression!)
let msg = if x > 0 { "positive" } else { "non-positive" };

// Loop variants
loop { break; }                    // infinite loop
while condition { }                // conditional loop
for item in collection { }        // iterator loop
for i in 0..10 { }                // range loop (0 to 9)
for (i, item) in v.iter().enumerate() { }  // with index

// Turbofish ::<T> — specify generic type explicitly
let v = Vec::<i32>::new();
let n = "42".parse::<u32>().unwrap();

// Macros (end with !)
println!("hello");     vec![1,2,3];     assert_eq!(a, b);
format!("x={}", x);   tracing::info!("msg");

// Attributes
#[derive(Debug)]       // on structs/enums: auto-implement traits
#[allow(dead_code)]    // suppress "unused" warnings
#[cfg(test)]           // conditional compilation (only in test builds)
#[test]                // marks a test function
#[inline]              // suggest inlining to compiler
```

---

## Exercises

Try these after reading the codebase:

1. **Add a new letter sequence** — Edit `letter_assignment.rs` to use a different key layout
   (e.g., Dvorak home row). Run `cargo test letter_assignment` to verify.

2. **Add a "window count" display** — In `overlay_renderer.rs`, add text showing "5 windows"
   in the top-right corner. Use the existing `text_brush` and `title_format`.

3. **Make fade duration configurable** — Add a `fade_duration_ms: u32` field to `AppConfig`,
   then use it in `animation.rs` to compute `ALPHA_DELTA` dynamically.

4. **Add a test** — Write a test in `grid_layout.rs` that verifies 3 windows produce a
   2x2 grid (2 cols, 2 rows). Follow the existing test style.

5. **Implement a "close window" action** — Add a `KeyAction::Close(HWND)` variant triggered
   by pressing Delete while a window is selected. You'll need to modify `interaction.rs`
   and `main.rs`.