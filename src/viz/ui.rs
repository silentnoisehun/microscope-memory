use crate::{MicroscopeReader, LAYER_NAMES};

pub struct UiState {
    pub depth_visible: [bool; 9],
    pub layer_visible: [bool; 10],
    pub selected_block: Option<usize>,
    pub selected_text: String,
    pub search_query: String,
    pub search_results: Vec<(usize, String)>,
    pub show_edges: bool,
    pub point_count: usize,
    pub fps: f32,
    pub needs_rebuild: bool,
}

impl UiState {
    pub fn new() -> Self {
        let mut depth_visible = [false; 9];
        // Show D0-D3 by default
        depth_visible[0] = true;
        depth_visible[1] = true;
        depth_visible[2] = true;
        depth_visible[3] = true;

        Self {
            depth_visible,
            layer_visible: [true; 10],
            selected_block: None,
            selected_text: String::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            show_edges: false,
            point_count: 0,
            fps: 0.0,
            needs_rebuild: false,
        }
    }
}

pub fn draw_panels(ctx: &egui::Context, state: &mut UiState, reader: Option<&MicroscopeReader>) {
    // Left panel — controls
    egui::SidePanel::left("controls").default_width(200.0).show(ctx, |ui| {
        ui.heading("Microscope");
        ui.separator();

        ui.label(format!("Points: {}", state.point_count));
        ui.label(format!("FPS: {:.0}", state.fps));
        ui.separator();

        // Depth visibility
        ui.label("Depth Levels:");
        for d in 0..9 {
            let label = format!("D{}", d);
            if ui.checkbox(&mut state.depth_visible[d], label).changed() {
                state.needs_rebuild = true;
            }
        }
        ui.separator();

        // Layer visibility
        ui.collapsing("Layers", |ui| {
            for (i, &name) in LAYER_NAMES.iter().enumerate() {
                if ui.checkbox(&mut state.layer_visible[i], name).changed() {
                    state.needs_rebuild = true;
                }
            }
        });
        ui.separator();

        // Edge toggle
        if ui.checkbox(&mut state.show_edges, "Show edges").changed() {
            state.needs_rebuild = true;
        }

        ui.separator();

        // Search
        ui.label("Search:");
        let search_response = ui.text_edit_singleline(&mut state.search_query);
        if search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(reader) = reader {
                let results = reader.find_text(&state.search_query, 10);
                state.search_results = results.iter().map(|&(_d, idx)| {
                    let preview: String = reader.text(idx).chars().take(40).collect();
                    (idx, preview)
                }).collect();
            }
        }

        if !state.search_results.is_empty() {
            ui.separator();
            ui.label("Results:");
            let mut clicked_idx = None;
            for (idx, preview) in &state.search_results {
                if ui.button(format!("#{}: {}", idx, preview)).clicked() {
                    clicked_idx = Some(*idx);
                }
            }
            if let Some(idx) = clicked_idx {
                state.selected_block = Some(idx);
                if let Some(reader) = reader {
                    state.selected_text = reader.text(idx).to_string();
                }
            }
        }
    });

    // Right panel — block inspector
    if state.selected_block.is_some() {
        egui::SidePanel::right("inspector").default_width(280.0).show(ctx, |ui| {
            ui.heading("Block Inspector");
            ui.separator();

            if let (Some(idx), Some(reader)) = (state.selected_block, reader) {
                let h = reader.header(idx);
                // Copy fields from packed struct to avoid unaligned reference UB
                let depth = h.depth;
                let layer_id = h.layer_id;
                let x = h.x;
                let y = h.y;
                let z = h.z;
                let parent_idx = h.parent_idx;
                let child_count = h.child_count;
                let layer_name = LAYER_NAMES.get(layer_id as usize).unwrap_or(&"?");

                ui.label(format!("Index: {}", idx));
                ui.label(format!("Depth: D{}", depth));
                ui.label(format!("Layer: {} ({})", layer_name, layer_id));
                ui.label(format!("Position: ({:.4}, {:.4}, {:.4})", x, y, z));
                ui.label(format!("Parent: {}", if parent_idx == u32::MAX { "root".to_string() } else { format!("#{}", parent_idx) }));
                ui.label(format!("Children: {}", child_count));
                ui.separator();

                ui.label("Content:");
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    ui.label(&state.selected_text);
                });
            }

            if ui.button("Close").clicked() {
                state.selected_block = None;
                state.selected_text.clear();
            }
        });
    }
}
