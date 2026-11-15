use photocraft_engine::Session;
use photocraft_ui_egui::{FileDialogAnswer, FileDialogRequest, PhotocraftApp, Services};
use winit::platform::android::activity::AndroidApp;

// SAFETY: android-activity requires this unique symbol with its documented Rust ABI.
// The fork owner approved this export-only exception; there are no unsafe operations here.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let data = app.internal_data_path();
    let activity = app.clone();
    let options = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    if let Err(error) = eframe::run_native(
        "PhotoCraft Mobile",
        options,
        Box::new(move |cc| {
            let pending: super::gallery::Pending = Default::default();
            let picker = pending.clone();
            let services = Services {
                import: Some(Box::new(|name, bytes, options| {
                    photocraft_io::import_with_options(name, bytes, options).map(|result| (result.document, result.warnings)).map_err(|error| error.to_string())
                })),
                export: Some(Box::new(|document, name, settings| {
                    photocraft_io::export(document, name, &settings.io_options())
                        .map(|result| (result.bytes, result.warnings))
                        .map_err(|error| error.to_string())
                })),
                file_dialog: Some(Box::new(move |request, _parent, reply| match request {
                    FileDialogRequest::Save { suggested } => reply.send(Some(FileDialogAnswer::SaveTo(suggested))),
                    FileDialogRequest::Open { multiple, .. } => {
                        *picker.borrow_mut() = Some((multiple, reply));
                    }
                    FileDialogRequest::Folder { .. } => {
                        reply.send(Some(FileDialogAnswer::Unavailable("Folder access is not available in this Android preview.".into())))
                    }
                })),
                write: Some(Box::new(super::content::write)),
                ..super::storage::services(data.ok_or("Android private storage is unavailable")?)?
            };
            let mut editor = PhotocraftApp::new(Session::new(), services);
            if let Some(state) = &cc.wgpu_render_state {
                editor.set_wgpu(state.clone());
            }
            match super::content::incoming() {
                Ok(Some((name, bytes))) => {
                    if let Err(error) = editor.open_bytes(&name, &bytes) {
                        editor.ui.status = error;
                    }
                }
                Ok(None) => {}
                Err(error) => editor.ui.status = error,
            }
            Ok(Box::new(super::gallery::Host { editor, activity, pending, photos: None, error: String::new(), next_poll: 0.0 }))
        }),
    ) {
        eprintln!("PhotoCraft could not start: {error}");
    }
}
