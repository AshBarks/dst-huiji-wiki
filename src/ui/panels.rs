use super::App;

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
                        self.spawn_file_load(path, None);
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

                    #[cfg(feature = "gif")]
                    {
                        let has_animation = self.get_current_animation().is_some();
                        ui.add_enabled_ui(has_animation, |ui| {
                            let exporting = self.gif_export.is_some()
                                || self.png_export.is_some();
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

                            let png_exporting = self.gif_export.is_some()
                                || self.png_export.is_some();
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
                    }
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

        let build_summaries: Vec<(String, Option<usize>, bool)> = self
            .builds
            .iter()
            .map(|entry| {
                let has_build = entry.build.is_some();
                (entry.source_name.clone(), entry.assigned_atlas, has_build)
            })
            .collect();

        let builds_len = self.builds.len();
        let mut new_enabled: Vec<(usize, bool)> = Vec::new();
        let mut swap_actions: Vec<(usize, usize)> = Vec::new();
        let mut remove_indices: Vec<usize> = Vec::new();
        let mut symbol_toggles: Vec<(usize, String, bool)> = Vec::new();
        let mut browse_dyn: Option<usize> = None;
        let mut browse_zip: Option<usize> = None;
        let mut need_re_render = false;

        for (idx, (source_name, assigned, has_build)) in build_summaries.iter().enumerate() {
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
                            for tm in &atlas_entry.tex_meta {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ({}x{} {:?})",
                                        tm.name, tm.width, tm.height, tm.pixel_format
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
                            for tm in &ae.tex_meta {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ({}x{} {:?})",
                                        tm.name, tm.width, tm.height, tm.pixel_format
                                    ))
                                    .small()
                                    .color(egui::Color32::LIGHT_BLUE),
                                );
                            }
                        }

                        if let Some(build) = &self.builds[idx].build {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Build: {} (v{})",
                                    build.name, build.version
                                ))
                                .small()
                                .italics(),
                            );

                            if !build.atlases.is_empty() {
                                egui::CollapsingHeader::new(
                                    egui::RichText::new(format!(
                                        "Atlases ({})",
                                        build.atlases.len()
                                    ))
                                    .small(),
                                )
                                .id_salt(format!("build_{idx}_atlases"))
                                .default_open(false)
                                .show(ui, |ui| {
                                    for atlas in &build.atlases {
                                        ui.label(
                                            egui::RichText::new(format!("  {}", atlas.name))
                                                .small(),
                                        );
                                    }
                                });
                            }

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("Symbols ({})", build.symbols.len()))
                                    .small(),
                            )
                            .id_salt(format!("build_{idx}_symbols"))
                            .default_open(false)
                            .show(ui, |ui| {
                                for symbol in &build.symbols {
                                    let key = symbol.name.to_lowercase();
                                    let is_enabled =
                                        !self.builds[idx].disabled_symbols.contains(&key);
                                    let mut toggle = is_enabled;
                                    if ui
                                        .checkbox(
                                            &mut toggle,
                                            format!(
                                                "{} ({} frames)",
                                                symbol.name,
                                                symbol.frames.len()
                                            ),
                                        )
                                        .changed()
                                    {
                                        symbol_toggles.push((idx, key, toggle));
                                    }
                                }
                            });
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
        }
        for (idx, key, enabled) in symbol_toggles {
            if enabled {
                self.builds[idx].disabled_symbols.remove(&key);
            } else {
                self.builds[idx].disabled_symbols.insert(key);
            }
            need_re_render = true;
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
                    self.show_symbol_dependencies(ui);
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
        ui.label(egui::RichText::new("Version").strong());
        ui.label(format!("  {}", anim.version));
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

    pub fn show_symbol_dependencies(&mut self, ui: &mut egui::Ui) {
        ui.heading("Symbol Dependencies");

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

        let mut symbol_names: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for frame in &animation.frames {
            for elem in &frame.elements {
                symbol_names.insert(elem.symbol_lower.clone());
            }
        }

        if symbol_names.is_empty() {
            ui.label("No symbols referenced");
            return;
        }

        let eligible_builds: Vec<(usize, &str, bool)> = self
            .builds
            .iter()
            .enumerate()
            .filter(|(_, e)| e.enabled && e.build.is_some() && e.assigned_atlas.is_some())
            .map(|(idx, e)| (idx, e.source_name.as_str(), false))
            .collect();

        for sym_lower in &symbol_names {
            let mut winner_build_idx: Option<usize> = None;
            let mut all_providers: Vec<(usize, &str)> = Vec::new();

            for &(build_idx, source_name, _) in &eligible_builds {
                let build = self.builds[build_idx].build.as_ref().unwrap();
                if build.symbol_index.contains_key(sym_lower.as_str()) {
                    all_providers.push((build_idx, source_name));
                }
            }

            for &(build_idx, _, _) in eligible_builds.iter().rev() {
                let entry = &self.builds[build_idx];
                if entry.disabled_symbols.contains(sym_lower.as_str()) {
                    continue;
                }
                let build = entry.build.as_ref().unwrap();
                if build.symbol_index.contains_key(sym_lower.as_str()) {
                    winner_build_idx = Some(build_idx);
                    break;
                }
            }

            egui::CollapsingHeader::new(sym_lower.as_str())
                .id_salt(format!("sym_dep_{}", sym_lower))
                .default_open(false)
                .show(ui, |ui| {
                    if all_providers.is_empty() {
                        ui.label(
                            egui::RichText::new("missing — no build provides this symbol")
                                .small()
                                .color(egui::Color32::RED),
                        );
                        return;
                    }

                    for (build_idx, source_name) in &all_providers {
                        let build = self.builds[*build_idx].build.as_ref().unwrap();
                        let sym = build
                            .symbol_index
                            .get(sym_lower.as_str())
                            .and_then(|&si| build.symbols.get(si));
                        let frame_count = sym.map(|s| s.frames.len()).unwrap_or(0);
                        let is_disabled = self.builds[*build_idx]
                            .disabled_symbols
                            .contains(sym_lower.as_str());
                        let is_winner = winner_build_idx == Some(*build_idx);

                        let (color, suffix) = if is_disabled {
                            (egui::Color32::YELLOW, " (disabled)")
                        } else if is_winner {
                            (egui::Color32::LIGHT_BLUE, "")
                        } else {
                            (egui::Color32::GRAY, " (shadowed)")
                        };

                        ui.label(
                            egui::RichText::new(format!(
                                "  {} [{} frames] from {}{}",
                                if is_winner { "\u{2500}" } else { "\u{2514}" },
                                frame_count,
                                source_name,
                                suffix,
                            ))
                            .small()
                            .color(color),
                        );
                    }
                });
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
