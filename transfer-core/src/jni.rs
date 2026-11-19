//! Android entry points. Kotlin owns SAF and file descriptors; Rust owns sockets and bytes.

use crate::fs::{Created, FileMeta, Fs, Hooks, Opened};
use crate::http::{serve_with_shutdown, AppState};
use jni::{
    objects::{GlobalRef, JClass, JObject, JString, JValue},
    sys::jlong,
    JNIEnv, JavaVM,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    os::fd::FromRawFd,
    sync::{Arc, OnceLock},
};

static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
static CALLBACK: OnceLock<GlobalRef> = OnceLock::new();

#[derive(Deserialize)]
struct JniConfig {
    port: u16,
    password: Option<String>,
    #[serde(default)]
    assets: HashMap<String, String>,
}

struct ServerHandle {
    shutdown: tokio::sync::oneshot::Sender<()>,
    thread: std::thread::JoinHandle<()>,
}

type Bridge = (&'static JavaVM, &'static GlobalRef);

fn bridge() -> Result<Bridge, String> {
    let vm = JAVA_VM.get().ok_or("server not initialised")?;
    let cb = CALLBACK.get().ok_or("callback not set")?;
    Ok((vm, cb))
}

fn invoke_string(method: &str, arg: Option<&str>) -> Result<String, String> {
    let (vm, cb) = bridge()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("{method}: attach: {e}"))?;
    let jarg = match arg {
        Some(value) => Some(
            env.new_string(value)
                .map_err(|e| format!("{method}: arg: {e}"))?,
        ),
        None => None,
    };
    let result = match &jarg {
        Some(value) => env.call_method(
            cb.as_obj(),
            method,
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(value)],
        ),
        None => env.call_method(
            cb.as_obj(),
            method,
            "()Ljava/lang/String;",
            &[],
        ),
    }
    .map_err(|e| format!("{method}: {e}"))?;

    let obj = result.l().map_err(|e| format!("{method}: {e}"))?;
    if obj.is_null() {
        return Err(format!("{method}: null"));
    }
    let text: String = env
        .get_string(&JString::from(obj))
        .map_err(|e| format!("{method}: {e}"))?
        .into();
    Ok(text)
}

fn file_from_fd(fd: i32) -> Result<File, String> {
    if fd < 0 {
        return Err("could not open file".into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

struct JniFs;

impl Fs for JniFs {
    fn list(&self) -> Result<Vec<FileMeta>, String> {
        let raw = invoke_string("listFiles", None)?;
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        Ok(entries
            .into_iter()
            .filter_map(|e| {
                Some(FileMeta {
                    name: e.get("name")?.as_str()?.to_string(),
                    size: e.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                    mtime: e.get("mtime").and_then(|v| v.as_u64()).unwrap_or(0),
                    mime: e
                        .get("mime")
                        .and_then(|v| v.as_str())
                        .unwrap_or("application/octet-stream")
                        .to_string(),
                })
            })
            .collect())
    }

    fn open_read(&self, name: &str) -> Result<Opened, String> {
        let raw = invoke_string("openRead", Some(name))?;
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            return Err(error.to_string());
        }
        if let Some(error) = value.get("msg").and_then(|v| v.as_str()) {
            return Err(error.to_string());
        }
        let fd = value
            .get("fd")
            .and_then(|v| v.as_i64())
            .ok_or("openRead: missing fd")? as i32;
        Ok(Opened {
            file: file_from_fd(fd)?,
            len: value
                .get("size")
                .and_then(|v| v.as_u64())
                .filter(|&n| n > 0),
            mime: value
                .get("mime")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string(),
        })
    }

    fn create(&self, name: &str) -> Result<Created, String> {
        let raw = invoke_string("create", Some(name))?;
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
            return Err(error.to_string());
        }
        let fd = value
            .get("fd")
            .and_then(|v| v.as_i64())
            .ok_or("create: missing fd")? as i32;
        Ok(Created {
            name: value
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("create: missing name")?
                .to_string(),
            file: file_from_fd(fd)?,
        })
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        let msg = invoke_string("delete", Some(name))?;
        if msg.is_empty() {
            Ok(())
        } else {
            Err(msg)
        }
    }
}

struct JniHooks;

impl Hooks for JniHooks {
    fn approve_ip(&self, ip: &str) -> bool {
        let (vm, cb) = match bridge() {
            Ok(b) => b,
            Err(_) => return false,
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let Ok(jarg) = env.new_string(ip) else {
            return false;
        };
        env.call_method(
            cb.as_obj(),
            "approveIp",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&jarg)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn on_change(&self) {
        let Ok((vm, cb)) = bridge() else {
            return;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let _ = env.call_method(cb.as_obj(), "onFileChanged", "()V", &[]);
    }
}

/// `RustServer.start0(configJson, callback) -> handle`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_transfer_server_RustServer_start0<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    config: JString<'local>,
    callback: JObject<'local>,
) -> jlong {
    match start_impl(&mut env, config, callback) {
        Ok(handle) => Box::into_raw(Box::new(handle)) as jlong,
        Err(e) => {
            let class = if e.starts_with("bind") {
                "java/net/BindException"
            } else {
                "java/lang/RuntimeException"
            };
            let _ = env.throw_new(class, e);
            0
        }
    }
}

/// `RustServer.stop0(handle)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_transfer_server_RustServer_stop0(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle == 0 {
        return;
    }
    // Safety: the handle came from start0 and is consumed exactly once.
    let handle = unsafe { Box::from_raw(handle as *mut ServerHandle) };
    let _ = handle.shutdown.send(());
    let _ = handle.thread.join();
}

fn start_impl(
    env: &mut JNIEnv,
    config: JString,
    callback: JObject,
) -> Result<ServerHandle, String> {
    let config: String = env
        .get_string(&config)
        .map_err(|e| format!("config: {e}"))?
        .into();
    let config: JniConfig = serde_json::from_str(&config).map_err(|e| format!("config: {e}"))?;

    let _ = JAVA_VM.set(env.get_java_vm().map_err(|e| format!("vm: {e}"))?);
    if CALLBACK.get().is_none() {
        let global = env
            .new_global_ref(callback)
            .map_err(|e| format!("callback: {e}"))?;
        let _ = CALLBACK.set(global);
    }

    let listener =
        crate::http::bind(config.port).map_err(|e| format!("bind {}: {e}", config.port))?;

    let state = Arc::new(AppState {
        fs: Arc::new(JniFs),
        hooks: Arc::new(JniHooks),
        password: config.password,
        assets: config
            .assets
            .into_iter()
            .map(|(k, v)| (k, v.into_bytes()))
            .collect(),
    });

    let (tx, rx) = tokio::sync::oneshot::channel();
    let thread = std::thread::Builder::new()
        .name("transfer-core".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            else {
                return;
            };
            let _ = runtime.block_on(serve_with_shutdown(listener, state, async {
                let _ = rx.await;
            }));
        })
        .map_err(|e| format!("spawn: {e}"))?;

    Ok(ServerHandle {
        shutdown: tx,
        thread,
    })
}
