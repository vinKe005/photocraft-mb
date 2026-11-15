//! In-app photo selection over Android's granted MediaStore images.
use crate::content;
use photocraft_ui_egui::{FileDialogAnswer, FileDialogReply, PhotocraftApp};
use std::cell::RefCell;
use std::rc::Rc;

pub(super) type Pending = Rc<RefCell<Option<(bool, FileDialogReply)>>>;

pub(super) struct Host {
    pub editor: PhotocraftApp,
    pub activity: winit::platform::android::activity::AndroidApp,
    pub pending: Pending,
    pub photos: Option<Vec<content::Photo>>,
    pub error: String,
    pub next_poll: f64,
}

impl eframe::App for Host {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::logic(&mut self.editor, ctx, frame);
        if self.pending.borrow().is_none() {
            self.photos = None;
            self.error.clear();
            return;
        }
        let now = ctx.input(|input| input.time);
        if self.photos.is_none() && now >= self.next_poll {
            self.next_poll = now + 0.5;
            match content::photos() {
                Ok(photos) => self.photos = photos,
                Err(error) => self.error = error,
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        // NativeActivity supplies the visible content bounds, including system bars/keyboard.
        let content = self.activity.content_rect();
        let scale = ctx.pixels_per_point();
        if content.right > content.left && content.bottom > content.top && scale.is_finite() && scale > 0.0 {
            let screen = input.screen_rect.unwrap_or_else(|| ctx.input(|input| input.viewport_rect()));
            input.safe_area_insets = Some(egui::SafeAreaInsets(egui::epaint::MarginF32 {
                left: (content.left as f32 / scale - screen.left()).max(0.0),
                top: (content.top as f32 / scale - screen.top()).max(0.0),
                right: (screen.right() - content.right as f32 / scale).max(0.0),
                bottom: (screen.bottom() - content.bottom as f32 / scale).max(0.0),
            }));
        }
        eframe::App::raw_input_hook(&mut self.editor, ctx, input);
    }

    fn on_exit(&mut self) {
        eframe::App::on_exit(&mut self.editor);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        eframe::App::ui(&mut self.editor, ui, frame);
        if self.pending.borrow().is_none() {
            return;
        }
        let mut choice = None;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("android_photos")).show(ui.ctx(), |ui| {
            ui.set_max_width((ui.ctx().content_rect().width() - 32.0).max(0.0).min(480.0));
            ui.spacing_mut().interact_size.y = 48.0;
            ui.heading("Fotos");
            ui.label("Selecione uma foto para editar.");
            if let Some(photos) = &self.photos {
                if photos.is_empty() {
                    ui.label("Nenhuma foto disponível. Verifique o acesso nas configurações do Android.");
                }
                egui::ScrollArea::vertical().max_height(ui.ctx().content_rect().height() * 0.5).show(ui, |ui| {
                    for (index, photo) in photos.iter().enumerate() {
                        if ui.add_sized([ui.available_width(), 48.0], egui::Button::new(&photo.name)).clicked() {
                            choice = Some(index);
                        }
                    }
                });
                ui.label("Até 256 fotos recentes são exibidas.");
            } else {
                ui.label("Permita o acesso às fotos que deseja editar. No Android 14 ou superior, você pode liberar somente fotos selecionadas.");
            }
            if ui.button("Selecionar fotos / alterar acesso").clicked() {
                self.photos = None;
                self.error.clear();
                if let Err(error) = content::request_photos() {
                    self.error = error;
                }
            }
            if !self.error.is_empty() {
                ui.label(&self.error);
            }
            cancel = ui.button("Cancelar").clicked();
        });
        if cancel || response.should_close() {
            if let Some((_, reply)) = self.pending.borrow_mut().take() {
                reply.send(None);
            }
        } else if let Some(index) = choice {
            if let Some(photo) = self.photos.as_ref().and_then(|photos| photos.get(index)) {
                if let Some((multiple, reply)) = self.pending.borrow_mut().take() {
                    let uri = photo.uri.clone();
                    let name = photo.name.clone();
                    self.editor.ui.status = "Abrindo foto…".into();
                    // File providers may block: keep the Android event/render thread responsive.
                    let launched = std::thread::Builder::new().name("photo-import".into()).spawn(move || {
                        let answer = match content::read_photo(&uri) {
                            Ok(bytes) if multiple => FileDialogAnswer::ContentsMany(vec![(name, bytes)]),
                            Ok(bytes) => FileDialogAnswer::Contents(name, bytes),
                            Err(error) => FileDialogAnswer::Unavailable(error),
                        };
                        reply.send(Some(answer));
                    });
                    if let Err(error) = launched {
                        self.editor.ui.status = format!("Não foi possível abrir a foto: {error}");
                        self.editor.ui.status_error = true;
                    }
                }
            }
        }
    }
}
