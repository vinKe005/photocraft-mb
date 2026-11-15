//! App-private preferences and the editor's existing incremental recovery store.
use photocraft_format::RecoveryStore;
use photocraft_ui_egui::{Recoverable, Services};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub(super) fn services(root: PathBuf) -> Result<Services, String> {
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let prefs = root.join("preferences.json");
    let prefs_write = prefs.clone();
    let recovery = root.join("recovery");
    let recover_dir = recovery.clone();
    let store = Rc::new(RefCell::new(RecoveryStore::new(recovery)));
    let (enqueue, results, flush, discard) = (store.clone(), store.clone(), store.clone(), store.clone());
    Ok(Services {
        load_prefs: Some(Box::new(move || {
            let mut file = std::fs::File::open(&prefs).ok()?;
            let bytes = crate::stream::read_limited(&mut file, 1 << 20).ok()?;
            String::from_utf8(bytes).ok()
        })),
        save_prefs: Some(Box::new(move |text| {
            if text.len() > 1 << 20 {
                return Err("Preferences exceed the 1 MiB limit".into());
            }
            photocraft_format::atomic::atomic_write(&prefs_write, text.as_bytes()).map_err(|error| error.to_string())
        })),
        autosave: Some(Box::new(move |document, revision, path| {
            enqueue.try_borrow_mut().map_err(|_| "Recovery is busy")?.autosave_checked(document, revision, path.map(str::to_string))
        })),
        autosave_results: Some(Box::new(move || results.try_borrow_mut().map(|mut store| store.take_completed()).unwrap_or_default())),
        flush_autosaves: Some(Box::new(move || flush.try_borrow_mut().map(|mut store| store.flush()).unwrap_or_default())),
        discard_autosave: Some(Box::new(move |id| {
            if let Ok(mut store) = discard.try_borrow_mut() {
                let _ = store.discard(id);
            }
        })),
        recover: Some(Box::new(move || {
            photocraft_format::list_recovery(&recover_dir)
                .into_iter()
                .map(|entry| Recoverable {
                    key: entry.info.key.clone(),
                    name: entry.info.document_name.clone(),
                    path: entry.info.original_path.clone(),
                    load: Box::new(move || photocraft_format::recover(&entry).map_err(|error| error.to_string())),
                })
                .collect()
        })),
        adopt_autosave: Some(Box::new(move |id, key| {
            if let Ok(mut store) = store.try_borrow_mut() {
                store.adopt(id, key);
            }
        })),
        ..Default::default()
    })
}
