#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod engine;
mod storage;
use engine::{AppError, ExtractionResult, ImageInfo, Partial, Progress};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    status: String,
    progress: Progress,
    image: Option<ImageInfo>,
    destination: Option<String>,
    partial_path: Option<String>,
    result: Option<ExtractionResult>,
    error: Option<AppError>,
    logs: Vec<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: "idle".into(),
            progress: Progress::default(),
            image: None,
            destination: None,
            partial_path: None,
            result: None,
            error: None,
            logs: vec!["Xiso Harbor 1.0.2 • xdvdfs 0.8.3 • native arm64".into()],
        }
    }
}
struct Session {
    _instance_lock: fs::File,
    snapshot: Mutex<Snapshot>,
    source: Mutex<Option<PathBuf>>,
    parent: Mutex<Option<PathBuf>>,
    partial: Mutex<Option<Partial>>,
    cancel: Arc<AtomicBool>,
    busy: AtomicBool,
    recovery: PathBuf,
    started: Mutex<Option<Instant>>,
}
impl Session {
    fn update(&self, app: &tauri::AppHandle, f: impl FnOnce(&mut Snapshot)) {
        let mut s = self.snapshot.lock().unwrap();
        f(&mut s);
        if s.logs.len() > 60 {
            s.logs.drain(1..2);
        }
        let _ = app.emit("extraction-state", &*s);
    }
    fn acquire(&self) -> engine::Result<()> {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| {
                AppError::new(
                    "busy",
                    "An operation is already running.",
                    "Wait for it to finish or cancel extraction.",
                    "Session is busy.",
                )
            })?;
        self.cancel.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn save_partial(&self, partial: &Partial) -> engine::Result<()> {
        storage::save(&self.recovery, partial).map_err(|e| AppError::io(e, "recovery record"))?;
        *self.partial.lock().unwrap() = Some(partial.clone());
        Ok(())
    }
    fn clear_partial(&self) -> engine::Result<()> {
        match fs::remove_file(&self.recovery) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(AppError::io(e, "recovery record")),
        }
        *self.partial.lock().unwrap() = None;
        Ok(())
    }
}
#[tauri::command]
fn get_state(state: State<'_, Arc<Session>>) -> Snapshot {
    let mut snapshot = state.snapshot.lock().unwrap().clone();
    if let Some(started) = *state.started.lock().unwrap() {
        snapshot.progress.elapsed_seconds = started.elapsed().as_secs();
    }
    snapshot
}
async fn inspect(
    app: tauri::AppHandle,
    state: Arc<Session>,
    path: PathBuf,
) -> engine::Result<Option<ImageInfo>> {
    state.acquire()?;
    state.update(&app, |s| {
        s.status = "validating".into();
        s.error = None;
        s.image = None;
        s.result = None;
    });
    *state.source.lock().unwrap() = None;
    let worker = state.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        engine::scan(&path, worker.cancel.clone()).map(|p| p.info)
    })
    .await
    .unwrap_or_else(|e| Err(AppError::invalid(format!("Native validation worker: {e}"))));
    match &result {
        Ok(info) => {
            *state.source.lock().unwrap() = Some(PathBuf::from(&info.path));
            state.update(&app, |s| {
                s.status = "ready".into();
                s.image = Some(info.clone());
                s.logs.push(format!(
                    "Preflight passed: {} files; {} output bytes.",
                    info.total_files, info.total_bytes
                ));
            });
        }
        Err(error) => state.update(&app, |s| {
            s.status = if error.category == "cancelled" {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            s.error = Some(error.clone());
            s.logs.push(error.detail.clone());
        }),
    }
    state.busy.store(false, Ordering::SeqCst);
    result.map(Some)
}
#[tauri::command]
async fn choose_iso(
    app: tauri::AppHandle,
    state: State<'_, Arc<Session>>,
) -> engine::Result<Option<ImageInfo>> {
    if state.busy.load(Ordering::SeqCst) || state.partial.lock().unwrap().is_some() {
        return Err(AppError::new(
            "busy",
            "Finish the current operation first.",
            "Resolve any partial files, then choose another ISO.",
            "Selection is locked.",
        ));
    }
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::invalid("The main application window is unavailable."))?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Choose an Xbox disc image")
        .add_filter("Xbox disc images", &["iso", "xiso"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    inspect(app, state.inner().clone(), file.path().to_path_buf()).await
}
#[tauri::command]
async fn inspect_drop(
    app: tauri::AppHandle,
    state: State<'_, Arc<Session>>,
    path: String,
) -> engine::Result<Option<ImageInfo>> {
    if state.partial.lock().unwrap().is_some() {
        return Err(AppError::new(
            "partial",
            "Resolve the partial extraction first.",
            "Keep or delete its files below.",
            "Partial output is pending.",
        ));
    }
    inspect(app, state.inner().clone(), PathBuf::from(path)).await
}
#[tauri::command]
async fn choose_folder(
    app: tauri::AppHandle,
    state: State<'_, Arc<Session>>,
) -> engine::Result<Option<String>> {
    if state.busy.load(Ordering::SeqCst) {
        return Err(AppError::new(
            "busy",
            "Extraction is running.",
            "Wait or cancel before changing destination.",
            "Destination is locked.",
        ));
    }
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::invalid("The main application window is unavailable."))?;
    let Some(folder) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .set_title("Save extracted games to")
        .pick_folder()
        .await
    else {
        return Ok(None);
    };
    let path = folder
        .path()
        .canonicalize()
        .map_err(|e| AppError::io(e, "destination"))?;
    let display = path.to_string_lossy().into_owned();
    *state.parent.lock().unwrap() = Some(path);
    state.update(&app, |s| {
        s.destination = Some(display.clone());
    });
    Ok(Some(display))
}
#[tauri::command]
fn suggest_name(state: State<'_, Arc<Session>>, name: String) -> String {
    state
        .parent
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| engine::unique_name(p, &name))
        .unwrap_or(name)
}
#[tauri::command]
fn start_extraction(
    app: tauri::AppHandle,
    state: State<'_, Arc<Session>>,
    name: String,
) -> engine::Result<()> {
    if state.partial.lock().unwrap().is_some() {
        return Err(AppError::new(
            "partial",
            "Resolve the previous partial extraction first.",
            "Choose Keep Partial Files or Delete Partial Files.",
            "Pending partial output.",
        ));
    }
    let source = state
        .source
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::invalid("Choose an ISO first."))?;
    let parent = state.parent.lock().unwrap().clone().ok_or_else(|| {
        AppError::new(
            "destination",
            "Choose an output folder first.",
            "Click Choose Folder.",
            "No destination selected.",
        )
    })?;
    state.acquire()?;
    *state.started.lock().unwrap() = Some(Instant::now());
    state.update(&app, |s| {
        s.status = "validating".into();
        s.error = None;
        s.result = None;
        s.progress = Progress::default();
        s.logs
            .push("Rechecking source and destination before extraction.".into());
    });
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // A panic is a failure, never a success; retain the recovery record.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> engine::Result<ExtractionResult> {
                let plan = engine::scan(&source, state.cancel.clone())?;
                let (partial, final_path) =
                    engine::prepare(&parent, &name, plan.info.total_bytes, plan.info.total_files)?;
                if let Err(error) = state.save_partial(&partial) {
                    let _ = partial.delete();
                    return Err(error);
                }
                state.update(&app, |s| {
                    s.status = "extracting".into();
                    s.partial_path = Some(partial.path.to_string_lossy().into());
                    s.logs
                        .push("Native streaming extraction started. Copy buffer: 1 MiB.".into());
                });
                let result = engine::extract(plan, &partial, &final_path, &state.cancel, |p| {
                    state.update(&app, |s| s.progress = p)
                });
                if result.is_ok() {
                    if let Err(error) = state.clear_partial() {
                        *state.partial.lock().unwrap() = None;
                        state.update(&app, |s| {
                            s.logs.push(format!(
                                "Output committed; stale recovery record could not be removed: {}",
                                error.detail
                            ))
                        });
                    }
                }
                result
            },
        ))
        .unwrap_or_else(|_| {
            Err(AppError::new(
                "internal",
                "The extraction worker stopped unexpectedly.",
                "Keep or delete the partial files, then try another image.",
                "Native worker panicked; no success was reported.",
            ))
        });
        *state.started.lock().unwrap() = None;
        state.update(&app, |s| match result {
            Ok(result) => {s.status = "completed".into(); s.partial_path = None; s.logs.push(format!("Verified {} files and {} bytes. Final directory committed without replacement.", result.files, result.bytes)); s.result = Some(result);},
            Err(error) => {s.status = if error.category == "cancelled" {"cancelled"} else {"failed"}.into(); s.logs.push(error.detail.clone()); s.error = Some(error);},
        });
        state.busy.store(false, Ordering::SeqCst);
    });
    Ok(())
}
#[tauri::command]
fn cancel_extraction(state: State<'_, Arc<Session>>) {
    state.cancel.store(true, Ordering::SeqCst);
}
#[tauri::command]
async fn resolve_partial(
    app: tauri::AppHandle,
    state: State<'_, Arc<Session>>,
    delete: bool,
) -> engine::Result<()> {
    state.acquire()?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let partial = state
                .partial
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| AppError::invalid("No partial output is pending."))?;
            if delete {
                partial.delete()?;
            }
            state.clear_partial()?;
            state.update(&app, |s| {
                s.partial_path = None;
                s.error = None;
                s.status = if s.image.is_some() { "ready" } else { "idle" }.into();
                s.logs.push(
                    if delete {
                        "App-managed partial files deleted."
                    } else {
                        "Partial files kept at the displayed location."
                    }
                    .into(),
                );
            });
            Ok(())
        })();
        state.busy.store(false, Ordering::SeqCst);
        result
    })
    .await
    .unwrap_or_else(|e| Err(AppError::invalid(e)))
}
#[tauri::command]
fn open_output(state: State<'_, Arc<Session>>) -> engine::Result<()> {
    let s = state.snapshot.lock().unwrap();
    let path = s
        .result
        .as_ref()
        .map(|r| r.path.as_str())
        .or(s.partial_path.as_deref())
        .ok_or_else(|| AppError::invalid("No output folder is available."))?;
    if s.result.is_none() {
        let partial = state
            .partial
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::invalid("No managed partial folder is available."))?;
        partial.verify()?;
    }
    let meta = fs::symlink_metadata(path).map_err(|e| AppError::io(e, "Finder"))?;
    if !Path::new(path).is_absolute() || !meta.is_dir() {
        return Err(AppError::new(
            "safety",
            "The output location is no longer a regular folder.",
            "Inspect the location manually in Finder.",
            "Refused to reveal a non-directory or symlink.",
        ));
    }
    let status = std::process::Command::new("/usr/bin/open")
        // Reveal in Finder, never dispatch a bundle-shaped folder to Launch Services.
        .arg("-R")
        .arg(Path::new(path))
        .status()
        .map_err(|e| AppError::io(e, "Finder"))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(
            "Finder",
            "Finder could not open the folder.",
            "Use the displayed folder path in Finder.",
            status,
        ))
    }
}
#[tauri::command]
fn reset(app: tauri::AppHandle, state: State<'_, Arc<Session>>) -> engine::Result<()> {
    if state.busy.load(Ordering::SeqCst) || state.partial.lock().unwrap().is_some() {
        return Err(AppError::new(
            "busy",
            "Finish the current operation first.",
            "Resolve any partial files.",
            "Reset refused.",
        ));
    }
    *state.source.lock().unwrap() = None;
    state.update(&app, |s| {
        let destination = s.destination.clone();
        *s = Snapshot::default();
        s.destination = destination;
    });
    Ok(())
}
#[tauri::command]
fn copy_diagnostics(state: State<'_, Arc<Session>>, error: Option<AppError>) -> engine::Result<()> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;
    let snapshot = state.snapshot.lock().unwrap();
    let mut report = format!(
        "Xiso Harbor 1.0.2 / xdvdfs 0.8.3 / arm64\nState: {}\nFiles: {}/{}\nBytes: {}/{}\n",
        snapshot.status,
        snapshot.progress.files_done,
        snapshot.progress.total_files,
        snapshot.progress.bytes_done,
        snapshot.progress.total_bytes
    );
    if let Some(error) = error.as_ref().or(snapshot.error.as_ref()) {
        report.push_str(&format!(
            "Category: {}\n{}\n{}\n",
            error.category, error.message, error.detail
        ));
    }
    report.push_str(&snapshot.logs.join("\n"));
    let mut paths = vec![];
    if let Some(image) = &snapshot.image {
        paths.push(image.path.clone());
    }
    if let Some(path) = &snapshot.destination {
        paths.push(path.clone());
    }
    if let Some(path) = &snapshot.partial_path {
        paths.push(path.clone());
    }
    if let Some(result) = &snapshot.result {
        paths.push(result.path.clone());
    }
    paths.sort_by_key(|p| std::cmp::Reverse(p.len()));
    for path in paths {
        if !path.is_empty() {
            report = report.replace(&path, "[local path removed]");
        }
    }
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    if pasteboard.setString_forType(&NSString::from_str(&report), unsafe {
        NSPasteboardTypeString
    }) {
        Ok(())
    } else {
        Err(AppError::new(
            "clipboard",
            "Diagnostic details could not be copied.",
            "Select and copy the technical details manually.",
            "NSPasteboard refused the text.",
        ))
    }
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_local_data_dir()?;
            fs::create_dir_all(&data)?;
            let instance_lock = match storage::lock(&data) {
                Ok(lock) => lock,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    rfd::MessageDialog::new().set_title("Xiso Harbor is already open")
                        .set_description("Use the open Xiso Harbor window. Only one session can manage extraction and recovery at a time.").show();
                    std::process::exit(0);
                }
                Err(e) => return Err(e.into()),
            };
            let recovery = data.join("pending-extraction.json");
            let mut snapshot = Snapshot::default();
            let partial = match storage::load(&recovery) {
                Ok(partial) => partial,
                Err(error) => {
                    snapshot.logs.push(format!("The previous recovery record could not be read: {error}"));
                    snapshot.error = Some(AppError::new("recovery", "The previous recovery record could not be read.", "Inspect any .partial folders from your previous session before starting again. No files were deleted.", error));
                    None
                }
            };
            if let Some(p) = &partial {
                snapshot.status = "cancelled".into();
                snapshot.partial_path = Some(p.path.to_string_lossy().into());
                snapshot.error = Some(AppError::new(
                    "interrupted",
                    "A previous extraction was interrupted.",
                    "Keep or delete its partial files before starting another game.",
                    "Recovered an app-managed partial directory from the previous session.",
                ));
            }
            app.manage(Arc::new(Session {
                _instance_lock: instance_lock,
                snapshot: Mutex::new(snapshot),
                source: Mutex::new(None),
                parent: Mutex::new(None),
                partial: Mutex::new(partial),
                cancel: Arc::new(AtomicBool::new(false)),
                busy: AtomicBool::new(false),
                recovery,
                started: Mutex::new(None),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            choose_iso,
            inspect_drop,
            choose_folder,
            suggest_name,
            start_extraction,
            cancel_extraction,
            resolve_partial,
            open_output,
            copy_diagnostics,
            reset
        ])
        .build(tauri::generate_context!())
        .expect("Could not launch Xiso Harbor")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { ref api, .. } = event {
                let state = app.state::<Arc<Session>>();
                if state.busy.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    let _ = app.emit("close-blocked", ());
                }
            }
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } = event
            {
                let state = app.state::<Arc<Session>>();
                if state.busy.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = app.emit("close-blocked", ());
                }
            }
        });
}
