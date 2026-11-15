//! Touch chrome for the Android host, independent of desktop dock preferences.
use crate::{PhotocraftApp, Tool, theme::Tokens};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MobileUi {
    /// Enable the phone layout in offscreen previews, independent of the target OS.
    #[serde(default)]
    pub enabled: bool,
    pub layers_open: bool,
    pub options_open: bool,
}

impl MobileUi {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

const TOUCH_TARGET: f32 = 48.0;

/// Height reserved for Layers while retaining a usable canvas.
pub fn layer_sheet_height(height: f32) -> f32 {
    // Leave space for the canvas even in landscape and with the keyboard visible.
    if height.is_finite() { (height * 0.45).clamp(0.0, 360.0) } else { 0.0 }
}

fn command(app: &mut PhotocraftApp, ui: &mut egui::Ui, label: &str, id: &str) {
    if ui.add_sized([TOUCH_TARGET, TOUCH_TARGET], egui::Button::new(label)).clicked()
        && let Err(error) = app.run(id, json!({}))
    {
        app.ui.status = error;
        app.ui.status_error = true;
    }
}

pub(crate) fn chrome(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let tokens = Tokens::get(ui.ctx());
    egui::Panel::top("mobile_actions").resizable(false).frame(egui::Frame::NONE.fill(tokens.chrome)).show(ui, |ui| {
        egui::ScrollArea::horizontal().id_salt("mobile_actions_scroll").show(ui, |ui| {
            ui.horizontal(|ui| {
                command(app, ui, tl!("New…"), "file.new");
                command(app, ui, tl!("Open…"), "file.open");
                command(app, ui, tl!("Undo"), "edit.undo");
                command(app, ui, tl!("Redo"), "edit.redo");
                command(app, ui, tl!("Save As…"), "file.saveAs");
                command(app, ui, tl!("Export As…"), "file.export.exportAs");
                command(app, ui, tl!("Fit on Screen"), "view.fitOnScreen");
            });
        });
        if !app.ui.status.is_empty() {
            ui.label(&app.ui.status);
        }
    });
    if app.home_screen_up() {
        return;
    }
    egui::Panel::bottom("mobile_tools").resizable(false).frame(egui::Frame::NONE.fill(tokens.chrome)).show(ui, |ui| {
        egui::ScrollArea::horizontal().id_salt("mobile_tools_scroll").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = TOUCH_TARGET;
                if ui.add_sized([TOUCH_TARGET, TOUCH_TARGET], egui::Button::new(tl!("Layers")).selected(app.ui.mobile.layers_open)).clicked() {
                    app.ui.mobile.layers_open = !app.ui.mobile.layers_open;
                }
                if ui.add_sized([TOUCH_TARGET, TOUCH_TARGET], egui::Button::new(tl!("Options")).selected(app.ui.mobile.options_open)).clicked() {
                    app.ui.mobile.options_open = !app.ui.mobile.options_open;
                }
                for (tool, label) in [(Tool::Brush, tl!("Brush")), (Tool::Eraser, tl!("Eraser")), (Tool::Crop, tl!("Crop")), (Tool::Hand, tl!("Hand"))] {
                    if ui.add_sized([TOUCH_TARGET, TOUCH_TARGET], egui::Button::new(label).selected(app.ui.tool == tool)).clicked() {
                        crate::tool_memory::select(app, tool);
                    }
                }
                ui.menu_button(tl!("Tools"), |ui| {
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for tool in Tool::ALL {
                            if ui
                                .add_sized([TOUCH_TARGET, TOUCH_TARGET], egui::Button::new(crate::i18n::t(tool.label())).selected(app.ui.tool == tool))
                                .clicked()
                            {
                                crate::tool_memory::select(app, tool);
                                ui.close();
                            }
                        }
                    });
                });
            });
        });
    });
    if app.ui.mobile.layers_open {
        let height = layer_sheet_height(ui.available_height());
        egui::Panel::bottom("mobile_layers")
            .exact_size(height)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(tokens.chrome))
            .show(ui, |ui| super::layers::layers(app, ui));
    }
}
