use crate::ui::App;

type BankData = Vec<(usize, String, Vec<(usize, String)>)>;

impl App {
    pub fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open File").clicked()
                    && let Some(paths) = rfd::FileDialog::new()
                        .add_filter("DST Files", &["zip", "dyn", "bin"])
                        .pick_files()
                {
                    for path in paths {
                        if let Ok(data) = std::fs::read(&path) {
                            self.spawn_file_load(path, data);
                        }
                    }
                }

                ui.add_enabled_ui(self.rendered_image.is_some(), |ui| {
                    if ui.button("Export PNG").clicked()
                        && let Some(img) = &self.rendered_image
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .set_file_name("frame.png")
                            .save_file()
                        && let Err(e) = img.save(&path)
                    {
                        self.error_message = Some(format!("Failed to save PNG: {e}"));
                    }

                    let has_animation = self.get_current_animation().is_some();
                    ui.add_enabled_ui(has_animation, |ui| {
                        let exporting = self.gif_export.is_some() || self.png_export.is_some();
                        let label = if exporting {
                            "Exporting..."
                        } else {
                            "Export GIF"
                        };
                        if ui
                            .add_enabled(!exporting, egui::Button::new(label))
                            .on_hover_text(if exporting {
                                "Export in progress..."
                            } else {
                                "Export animation as GIF (builtin quantizer)"
                            })
                            .clicked()
                        {
                            self.start_gif_export();
                        }

                        let png_exporting = self.gif_export.is_some() || self.png_export.is_some();
                        let png_label = if png_exporting {
                            "Exporting..."
                        } else {
                            "Export PNG+GIF"
                        };
                        if ui
                            .add_enabled(!png_exporting, egui::Button::new(png_label))
                            .on_hover_text(if png_exporting {
                                "Export in progress..."
                            } else {
                                "Export PNG sequence, then convert to GIF via ffmpeg (better quality)"
                            })
                            .clicked()
                        {
                            self.start_png_export();
                        }
                    });
                });
            });
        });
    }

    pub fn show_left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("left_panel")
            .default_width(260.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_anim_section(ui);
                    ui.separator();
                    self.show_build_section(ui);
                });
            });
    }

    pub fn show_anim_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Animations");

        if self.anims.is_empty() {
            ui.label("No animations loaded");
            return;
        }

        let mut remove_idx: Option<usize> = None;
        let anim_summaries: Vec<(bool, String)> = self
            .anims
            .iter()
            .map(|e| (e.enabled, e.source_name.clone()))
            .collect();

        for (idx, (_enabled, source_name)) in anim_summaries.iter().enumerate() {
            let is_active = idx == self.active_anim_idx;
            let header = egui::RichText::new(source_name).color(if is_active {
                egui::Color32::from_rgb(100, 180, 255)
            } else {
                egui::Color32::PLACEHOLDER
            });

            egui::CollapsingHeader::new(header)
                .id_salt(format!("anim_{idx}"))
                .default_open(is_active)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut en = self.anims[idx].enabled;
                        if ui.checkbox(&mut en, "Enabled").changed() {
                            self.anims[idx].enabled = en;
                            self.needs_re_render = true;
                        }
                        if ui.small_button("X").clicked() {
                            remove_idx = Some(idx);
                        }
                    });

                    let bank_data: BankData = self.anims[idx]
                        .anim
                        .banks
                        .iter()
                        .enumerate()
                        .map(|(bi, b)| {
                            (
                                bi,
                                b.name.clone(),
                                b.animations
                                    .iter()
                                    .enumerate()
                                    .map(|(ai, a)| (ai, a.name.clone()))
                                    .collect(),
                            )
                        })
                        .collect();

                    for (bank_idx, bank_name, animations) in bank_data {
                        egui::CollapsingHeader::new(&bank_name)
                            .id_salt(format!("anim_{idx}_bank_{bank_idx}"))
                            .default_open(false)
                            .show(ui, |ui| {
                                for (anim_inner_idx, anim_name) in &animations {
                                    let is_selected = is_active
                                        && bank_idx == self.active_bank_idx
                                        && *anim_inner_idx == self.active_anim_inner_idx;
                                    if ui.selectable_label(is_selected, anim_name).clicked() {
                                        self.active_anim_idx = idx;
                                        self.active_bank_idx = bank_idx;
                                        self.active_anim_inner_idx = *anim_inner_idx;
                                        self.active_frame_idx = 0;
                                        self.playing = false;
                                        self.cache_dirty = true;
                                        self.needs_re_render = true;
                                    }
                                }
                            });
                    }
                });
        }

        if let Some(idx) = remove_idx {
            self.anims.remove(idx);
            if self.anims.is_empty() {
                self.active_anim_idx = 0;
                self.active_bank_idx = 0;
                self.active_anim_inner_idx = 0;
                self.active_frame_idx = 0;
                self.cache_dirty = true;
                self.needs_re_render = true;
            } else if idx <= self.active_anim_idx {
                self.active_anim_idx = self.active_anim_idx.saturating_sub(1);
                self.active_bank_idx = 0;
                self.active_anim_inner_idx = 0;
                self.active_frame_idx = 0;
                self.cache_dirty = true;
                self.needs_re_render = true;
            }
        }
    }

    pub fn show_build_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Builds");

        if self.builds.is_empty() {
            ui.label("No builds loaded");
            return;
        }

        let build_summaries: Vec<(bool, String, Option<usize>, bool)> = self
            .builds
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                let is_selected = self.selected_build_idx == Some(idx);
                let has_build = entry.build.is_some();
                (
                    is_selected,
                    entry.source_name.clone(),
                    entry.assigned_atlas,
                    has_build,
                )
            })
            .collect();

        let builds_len = self.builds.len();
        let mut new_enabled: Vec<(usize, bool)> = Vec::new();
        let mut swap_actions: Vec<(usize, usize)> = Vec::new();
        let mut remove_indices: Vec<usize> = Vec::new();
        let mut select_idx: Option<usize> = None;
        let mut browse_dyn: Option<usize> = None;
        let mut browse_zip: Option<usize> = None;
        let mut need_re_render = false;

        for (idx, (is_selected, source_name, assigned, has_build)) in
            build_summaries.iter().enumerate()
        {
            let atlas_name = assigned
                .and_then(|ai| self.atlas_entries.get(ai).and_then(|e| e.as_ref()))
                .map(|a| a.source_name.clone())
                .unwrap_or_default();

            let header = if *has_build && assigned.is_some() {
                if atlas_name.is_empty() {
                    source_name.clone()
                } else {
                    format!("{} [{}]", source_name, atlas_name)
                }
            } else if *has_build {
                format!("{} (pending atlas)", source_name)
            } else {
                format!("{} (pending build)", source_name)
            };

            let header_color = if !has_build || assigned.is_none() {
                egui::Color32::YELLOW
            } else if *is_selected {
                egui::Color32::from_rgb(100, 180, 255)
            } else {
                egui::Color32::PLACEHOLDER
            };

            egui::CollapsingHeader::new(egui::RichText::new(header).color(header_color))
                .id_salt(format!("build_{idx}"))
                .default_open(!*has_build || assigned.is_none())
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if *has_build {
                            let mut enabled = self.builds[idx].enabled;
                            if ui.checkbox(&mut enabled, "Enabled").changed() {
                                new_enabled.push((idx, enabled));
                            }
                        }
                        if *has_build && idx > 0 && ui.small_button("Up").clicked() {
                            swap_actions.push((idx, idx - 1));
                        }
                        if *has_build && idx + 1 < builds_len && ui.small_button("Down").clicked() {
                            swap_actions.push((idx, idx + 1));
                        }
                        if ui.small_button("X").clicked() {
                            remove_indices.push(idx);
                        }
                    });

                    if *has_build && assigned.is_none() {
                        ui.label(
                            egui::RichText::new("No atlas — needs a .dyn atlas file")
                                .small()
                                .color(egui::Color32::YELLOW),
                        );
                        if ui.small_button("Browse .dyn...").clicked() {
                            browse_dyn = Some(idx);
                        }
                    }

                    if !has_build && assigned.is_some() {
                        ui.label(
                            egui::RichText::new("No build — needs a .zip build file")
                                .small()
                                .color(egui::Color32::YELLOW),
                        );
                        if ui.small_button("Browse .zip...").clicked() {
                            browse_zip = Some(idx);
                        }
                        if let Some(atlas_idx) = assigned
                            && let Some(Some(atlas_entry)) = self.atlas_entries.get(*atlas_idx)
                        {
                            for (name, img) in &atlas_entry.decoded {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ({}x{})",
                                        name,
                                        img.width(),
                                        img.height()
                                    ))
                                    .small()
                                    .color(egui::Color32::LIGHT_BLUE),
                                );
                            }
                        }
                    }

                    if *has_build {
                        if let Some(atlas_idx) = assigned
                            && let Some(Some(ae)) = self.atlas_entries.get(*atlas_idx)
                        {
                            ui.label(
                                egui::RichText::new(format!("Atlas: {}", ae.source_name))
                                    .small()
                                    .italics(),
                            );
                        }

                        if let Some(build) = &self.builds[idx].build {
                            ui.label(
                                egui::RichText::new(format!("Build: {}", build.name))
                                    .small()
                                    .italics(),
                            );

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("Symbols ({})", build.symbols.len()))
                                    .small(),
                            )
                            .id_salt(format!("build_{idx}_symbols"))
                            .default_open(false)
                            .show(ui, |ui| {
                                for symbol in &build.symbols {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}  ({} frames)",
                                            symbol.name,
                                            symbol.frames.len(),
                                        ))
                                        .small(),
                                    );
                                }
                            });
                        }

                        if ui.small_button("Select").clicked() {
                            select_idx = Some(idx);
                        }
                    }
                });
        }

        for (idx, enabled) in new_enabled {
            self.builds[idx].enabled = enabled;
            need_re_render = true;
        }
        for (a, b) in swap_actions {
            self.builds.swap(a, b);
            need_re_render = true;
            if let Some(sel) = self.selected_build_idx {
                if sel == a {
                    self.selected_build_idx = Some(b);
                } else if sel == b {
                    self.selected_build_idx = Some(a);
                }
            }
        }
        remove_indices.sort();
        remove_indices.reverse();
        for idx in remove_indices {
            if let Some(atlas_idx) = self.builds[idx].assigned_atlas {
                let other_refs = self
                    .builds
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != idx)
                    .filter(|(_, b)| b.assigned_atlas == Some(atlas_idx))
                    .count();
                if other_refs == 0 {
                    self.atlas_entries[atlas_idx] = None;
                }
            }
            self.builds.remove(idx);
            need_re_render = true;
            if let Some(sel) = self.selected_build_idx {
                if sel == idx {
                    self.selected_build_idx = None;
                } else if sel > idx {
                    self.selected_build_idx = Some(sel - 1);
                }
            }
        }
        if let Some(idx) = select_idx {
            self.selected_build_idx = Some(idx);
        }

        if let Some(build_idx) = browse_dyn
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("DST Atlas", &["dyn"])
                .pick_file()
        {
            self.associate_dyn_to_build(build_idx, &path);
        }
        if let Some(build_idx) = browse_zip
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("DST Build", &["zip"])
                .pick_file()
        {
            self.associate_zip_to_pending_atlas(build_idx, &path);
        }

        if need_re_render {
            self.cache_dirty = true;
            self.needs_re_render = true;
        }

        if self.builds.is_empty() {
            ui.label("No builds loaded");
        }
    }

    pub fn show_right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("right_panel")
            .default_width(300.0)
            .min_width(240.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.show_anim_info(ui);
                    ui.separator();
                    self.show_build_info(ui);
                });
            });
    }

    pub fn show_anim_info(&mut self, ui: &mut egui::Ui) {
        ui.heading("Animation Info");
        let Some(anim_entry) = self.anims.get(self.active_anim_idx) else {
            ui.label("No animation loaded");
            return;
        };
        if !anim_entry.enabled {
            ui.label("Animation disabled");
            return;
        }

        let anim = &anim_entry.anim;
        let Some(bank) = anim.banks.get(self.active_bank_idx) else {
            ui.label("No bank selected");
            return;
        };
        let Some(animation) = bank.animations.get(self.active_anim_inner_idx) else {
            ui.label("No animation selected");
            return;
        };

        ui.label(egui::RichText::new("Source").strong());
        ui.label(format!("  {}", anim_entry.source_name));
        ui.label(egui::RichText::new("Bank").strong());
        ui.label(format!("  {}", bank.name));
        ui.label(egui::RichText::new("Animation").strong());
        ui.label(format!("  {}", animation.name));
        ui.label(format!("  Frame rate: {}", animation.frame_rate));
        ui.label(format!("  Total frames: {}", animation.frames.len()));

        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Current Frame {}", self.active_frame_idx)).strong());

        if let Some(frame) = animation.frames.get(self.active_frame_idx) {
            ui.label(format!(
                "  Bounding box: ({:.1}, {:.1}) {:.1} x {:.1}",
                frame.x, frame.y, frame.width, frame.height,
            ));
            ui.label(format!("  Elements: {}", frame.elements.len()));
            ui.label(format!("  Events: {}", frame.events.len()));

            if !frame.events.is_empty() {
                ui.label(egui::RichText::new("  Events:").italics());
                for event in &frame.events {
                    ui.label(egui::RichText::new(format!("    {}", event)).small());
                }
            }

            ui.add_space(2.0);
            egui::CollapsingHeader::new(format!("Elements ({})", frame.elements.len()))
                .id_salt("anim_info_elements")
                .default_open(false)
                .show(ui, |ui| {
                    for (i, elem) in frame.elements.iter().enumerate() {
                        egui::CollapsingHeader::new(format!(
                            "#{} z={:.1} {}",
                            i, elem.z_index, elem.symbol
                        ))
                        .id_salt(format!("anim_info_elem_{i}"))
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(format!("  symbol: {}", elem.symbol)).small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("  frameNum: {}", elem.frame_num))
                                    .small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("  layer: {}", elem.layer_name))
                                    .small(),
                            );
                            ui.label(
                                egui::RichText::new(format!("  zIndex: {:.2}", elem.z_index))
                                    .small(),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "  matrix: [{:.3}, {:.3}, {:.3}, {:.3}, {:.1}, {:.1}]",
                                    elem.a, elem.b, elem.c, elem.d, elem.tx, elem.ty
                                ))
                                .small(),
                            );
                        });
                    }
                });
        }
    }

    pub fn show_build_info(&mut self, ui: &mut egui::Ui) {
        ui.heading("Build Info");
        let Some(idx) = self.selected_build_idx else {
            ui.label("No build selected");
            return;
        };
        let Some(entry) = self.builds.get(idx) else {
            ui.label("Build not found");
            return;
        };

        ui.label(egui::RichText::new("Source").strong());
        ui.label(format!("  {}", entry.source_name));

        if let Some(build) = &entry.build {
            ui.label(egui::RichText::new("Build").strong());
            ui.label(format!("  Name: {}", build.name));
            ui.label(format!("  Version: {}", build.version));
            ui.label(format!("  Symbols: {}", build.symbols.len()));

            ui.label(egui::RichText::new("Atlas").strong());
            if let Some(ai) = entry.assigned_atlas {
                if let Some(Some(ae)) = self.atlas_entries.get(ai) {
                    let atlas_name = build
                        .atlases
                        .first()
                        .map(|a| a.name.as_str())
                        .unwrap_or("?");
                    ui.label(format!("  {} (from {})", atlas_name, ae.source_name));
                } else {
                    ui.label("  (invalid ref)");
                }
            } else {
                ui.label("  (none)");
            }

            ui.add_space(2.0);
            egui::CollapsingHeader::new(format!("Symbols ({})", build.symbols.len()))
                .id_salt("build_info_symbols")
                .default_open(false)
                .show(ui, |ui| {
                    for symbol in &build.symbols {
                        let frame_count = symbol.frames.len();
                        let has_image = symbol.frames.iter().any(|f| f.image.is_some());
                        let img_mark = if has_image { " [img]" } else { "" };
                        ui.label(
                            egui::RichText::new(format!(
                                "{}  ({} frames){}",
                                symbol.name, frame_count, img_mark
                            ))
                            .small(),
                        );
                    }
                });
        } else {
            ui.label(egui::RichText::new("Atlas").strong());
            if let Some(ai) = entry.assigned_atlas
                && let Some(Some(ae)) = self.atlas_entries.get(ai)
            {
                for (name, img) in &ae.decoded {
                    ui.label(format!("  {} ({}x{})", name, img.width(), img.height()));
                }
            }
            ui.label(
                egui::RichText::new("Pending — needs a .zip build file")
                    .color(egui::Color32::YELLOW),
            );
        }
    }

    pub fn show_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.frame_texture {
                let avail = ui.available_size();
                let img_size = texture.size_vec2();
                let scale_x = avail.x / img_size.x;
                let scale_y = avail.y / img_size.y;
                let scale = scale_x.min(scale_y).min(1.0);
                let display_size = img_size * scale;
                ui.centered_and_justified(|ui| {
                    ui.image((texture.id(), display_size));
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "No animation loaded.\nOpen .zip, .dyn or .bin files, or drag and drop.",
                        )
                        .text_style(egui::TextStyle::Heading)
                        .color(egui::Color32::GRAY),
                    );
                });
            }
        });
    }

    pub fn show_bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Prev").clicked() {
                    self.prev_frame();
                    self.playing = false;
                }

                let play_label = if self.playing { "Pause" } else { "Play" };
                if ui.button(play_label).clicked() {
                    self.playing = !self.playing;
                    self.last_frame_time = std::time::Instant::now();
                    if self.playing && self.bg_renderer.is_none() {
                        self.start_background_render();
                    }
                }

                if ui.button("Next").clicked() {
                    self.next_frame();
                    self.playing = false;
                }

                ui.separator();

                if let Some(anim) = self.get_current_animation() {
                    let total = anim.frames.len();
                    ui.label(format!("Frame {} / {}", self.active_frame_idx + 1, total));
                    ui.label(format!("  {} fps", anim.frame_rate));
                } else {
                    ui.label("No animation selected");
                }

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.play_speed, 0.1..=5.0)
                        .text("Speed")
                        .step_by(0.1),
                );

                if self.is_loading() {
                    ui.separator();
                    ui.spinner();
                    ui.label("Loading...");
                }
            });
        });
    }

    pub fn show_error_overlay(&mut self, ctx: &egui::Context) {
        if self.error_message.is_none() {
            return;
        }
        egui::Area::new(egui::Id::new("error_overlay"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 30.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, self.error_message.as_ref().unwrap());
                        if ui.button("Close").clicked() {
                            self.error_message = None;
                        }
                    });
                });
            });
    }
}
