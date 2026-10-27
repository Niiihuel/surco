//! The Android host: the activity's handle, one JNI door each way between Rust and the Kotlin
//! shell's `SurcoBridge`, and what the shell says of the window the app draws in.
//!
//! Rust asks Kotlin with [`call`], a kind and a JSON body, and has a JSON answer back. Kotlin
//! tells Rust with `SurcoBridge.nativeEvent(kind, json)`: a kind some module [`listen`]s for
//! goes to it at once, on Kotlin's thread, which is how the media buttons reach the player
//! with the screen off; any other waits for the window's next frame in [`take_events`].

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use android_activity::AndroidApp;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::{jni_sig, jni_str, Env, EnvUnowned, JavaVM};

/// The Kotlin class every call goes through; its package is fixed, whatever the app's id.
const BRIDGE_CLASS: &str = "io.github.niiihuel.surco.SurcoBridge";

static APP: OnceLock<AndroidApp> = OnceLock::new();
/// The window's context, which an event waiting for a frame asks for one.
static REPAINT: OnceLock<eframe::egui::Context> = OnceLock::new();
/// Found once on the activity's thread: other threads have no class loader that knows it.
static BRIDGE: OnceLock<Global<JClass<'static>>> = OnceLock::new();

type Listener = Box<dyn Fn(&str) + Send + Sync>;
static LISTENERS: Mutex<Vec<(&'static str, Listener)>> = Mutex::new(Vec::new());
static EVENTS: Mutex<VecDeque<(String, String)>> = Mutex::new(VecDeque::new());

/// What the system's bars, the cutout and the keyboard cover of the window, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
pub struct Insets { pub top: f32, pub bottom: f32, pub left: f32, pub right: f32, pub ime: f32 }
static INSETS: Mutex<Insets> = Mutex::new(Insets { top: 0.0, bottom: 0.0, left: 0.0, right: 0.0, ime: 0.0 });

/// Keeps the activity and readies JNI before anything else runs: the platform's certificate
/// checks, which every request over TLS goes through, and the bridge's class.
pub fn start(app: AndroidApp) -> Result<(), String> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
        let loader = env.call_method(&activity, jni_str!("getClassLoader"), jni_sig!("()Ljava/lang/ClassLoader;"), &[])?.l()?;
        let name = env.new_string(BRIDGE_CLASS)?;
        let class = env.call_method(&loader, jni_str!("loadClass"), jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)])?.l()?;
        let class = env.cast_local::<JClass>(class)?;
        let _ = BRIDGE.set(env.new_global_ref(&class)?);
        rustls_platform_verifier::android::init_with_env(env, activity)?;
        Ok(())
    }).map_err(|error| format!("Android bridge: {error}"))?;
    let _ = APP.set(app);
    listen("window.insets", |body| {
        if let Ok(insets) = serde_json::from_str::<Insets>(body) {
            *INSETS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = insets;
            if let Some(ctx) = REPAINT.get() { ctx.request_repaint(); }
        }
    });
    Ok(())
}

/// The window's insets, as the activity last said them.
pub fn insets() -> Insets {
    *INSETS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The window's context, once it is made: an event kept for a frame asks it for one.
pub fn set_repaint(ctx: &eframe::egui::Context) {
    let _ = REPAINT.set(ctx.clone());
}

/// The activity, once [`start`] has kept it.
pub fn app() -> Option<AndroidApp> {
    APP.get().cloned()
}

/// Where the app keeps what it has: its own files, which Android backs up and keeps until
/// the app is uninstalled.
pub fn data_dir() -> Option<PathBuf> {
    APP.get()?.internal_data_path()
}

/// Asks the Kotlin shell for `kind` with a JSON `body`; its JSON answer, or `None` if the
/// bridge is not ready or the call failed. Safe from any thread.
pub fn call(kind: &str, body: &str) -> Option<String> {
    let class = BRIDGE.get()?;
    let vm = JavaVM::singleton().ok()?;
    let answer = vm.attach_current_thread(|env| -> jni::errors::Result<Option<String>> {
        let kind = env.new_string(kind)?;
        let body = env.new_string(body)?;
        let answer = env.call_static_method(class, jni_str!("call"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
            &[JValue::Object(&kind), JValue::Object(&body)])?.l()?;
        if answer.is_null() { return Ok(None); }
        Ok(Some(env.cast_local::<JString>(answer)?.try_to_string(env)?))
    });
    answer.unwrap_or_else(|error| {
        eprintln!("Surco Android call {kind}: {error}");
        None
    })
}

/// Hands every event whose kind begins with `prefix` to `handler` as it arrives, on Kotlin's
/// thread, instead of keeping it for the next frame.
pub fn listen(prefix: &'static str, handler: impl Fn(&str) + Send + Sync + 'static) {
    LISTENERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push((prefix, Box::new(handler)));
}

/// The events no module listens for, as (kind, JSON), oldest first, since the last frame.
pub fn take_events() -> Vec<(String, String)> {
    EVENTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).drain(..).collect()
}

fn deliver(kind: String, body: String) {
    let listeners = LISTENERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, handler)) = listeners.iter().find(|(prefix, _)| kind.starts_with(prefix)) {
        handler(&body);
        return;
    }
    drop(listeners);
    EVENTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push_back((kind, body));
    // A frame to read it in: the window may be still.
    if let Some(ctx) = REPAINT.get() { ctx.request_repaint(); }
}

/// `SurcoBridge.nativeEvent(kind: String, json: String)`, Kotlin's one way in.
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_niiihuel_surco_SurcoBridge_nativeEvent<'caller>(
    mut unowned: EnvUnowned<'caller>, _class: JClass<'caller>, kind: JString<'caller>, body: JString<'caller>,
) {
    let outcome = unowned.with_env(|env: &mut Env| -> jni::errors::Result<Option<(String, String)>> {
        Ok(Some((kind.try_to_string(env)?, body.try_to_string(env)?)))
    });
    if let Some((kind, body)) = outcome.resolve::<jni::errors::LogErrorAndDefault>() {
        deliver(kind, body);
    }
}
