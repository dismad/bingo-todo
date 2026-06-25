use chrono::{DateTime, Utc};
use eframe::{egui, NativeOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Task {
    id: Uuid,
    description: String,
    completed: bool,
    notes: Option<String>,
    tags: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct DailyCard {
    id: Uuid,
    domain: String,
    date: String,
    tasks: Vec<Task>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Domain {
    name: String,
    accent: [u8; 3],
    template_tasks: Vec<Task>,
}

#[derive(Default, Serialize, Deserialize, Debug)]
struct AppData {
    domains: HashMap<String, Domain>,
    open_cards: Vec<DailyCard>,
    archived_cards: Vec<DailyCard>,
    current_card_index: Option<usize>,
}

impl AppData {
    fn default_domains() -> HashMap<String, Domain> {
        let mut map = HashMap::new();

        map.insert("Zcash Node Operations".to_string(), Domain {
            name: "Zcash Node Operations".to_string(),
            accent: [0, 150, 136],
            template_tasks: vec![
                Task { id: Uuid::new_v4(), description: "Check zebrad sync height and status".to_string(), completed: false, notes: None, tags: vec!["node".to_string()] },
                Task { id: Uuid::new_v4(), description: "Verify local shielded balance".to_string(), completed: false, notes: None, tags: vec!["shielded".to_string()] },
                Task { id: Uuid::new_v4(), description: "Review recent Orchard activity".to_string(), completed: false, notes: None, tags: vec!["orchard".to_string()] },
                Task { id: Uuid::new_v4(), description: "Check Grafana dashboards for anomalies".to_string(), completed: false, notes: None, tags: vec!["monitoring".to_string()] },
            ],
        });

        map.insert("Linux Server Maintenance".to_string(), Domain {
            name: "Linux Server Maintenance".to_string(),
            accent: [255, 152, 0],
            template_tasks: vec![
                Task { id: Uuid::new_v4(), description: "Run system updates and reboot if needed".to_string(), completed: false, notes: None, tags: vec!["update".to_string()] },
                Task { id: Uuid::new_v4(), description: "Check disk space and logs".to_string(), completed: false, notes: None, tags: vec!["storage".to_string()] },
                Task { id: Uuid::new_v4(), description: "Verify Tailscale and SSH access".to_string(), completed: false, notes: None, tags: vec!["network".to_string()] },
            ],
        });

        map.insert("Crypto Portfolio & Privacy".to_string(), Domain {
            name: "Crypto Portfolio & Privacy".to_string(),
            accent: [103, 58, 183],
            template_tasks: vec![
                Task { id: Uuid::new_v4(), description: "Review shielded ZEC balance and recent txs".to_string(), completed: false, notes: None, tags: vec!["privacy".to_string()] },
                Task { id: Uuid::new_v4(), description: "Check portfolio allocation and alerts".to_string(), completed: false, notes: None, tags: vec!["portfolio".to_string()] },
            ],
        });

        map
    }
}

struct BingoTodoApp {
    data: AppData,
    data_path: PathBuf,
    current_view: View,
    theme: Theme,
    ui_scale: f32,
    new_task_text: String,
    new_tag_text: String,
    status_message: String,
    visible_domains: HashMap<String, bool>,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum View {
    Daily,
    Performance,
}

#[derive(PartialEq, Clone, Copy)]
enum Theme {
    Dark,
    Light,
}

impl Default for BingoTodoApp {
    fn default() -> Self {
        let proj_dirs = directories::ProjectDirs::from("dev", "dismad", "bingo-todo").expect("dirs");
        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(data_dir).ok();
        let data_path = data_dir.join("app_data.json");

        let data = if data_path.exists() {
            match fs::read_to_string(&data_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| AppData {
                    domains: AppData::default_domains(),
                    open_cards: vec![],
                    archived_cards: vec![],
                    current_card_index: None,
                }),
                Err(_) => AppData {
                    domains: AppData::default_domains(),
                    open_cards: vec![],
                    archived_cards: vec![],
                    current_card_index: None,
                },
            }
        } else {
            AppData {
                domains: AppData::default_domains(),
                open_cards: vec![],
                archived_cards: vec![],
                current_card_index: None,
            }
        };

        Self {
            data,
            data_path,
            current_view: View::Daily,
            theme: Theme::Dark,
            ui_scale: 1.15,
            new_task_text: String::new(),
            new_tag_text: String::new(),
            status_message: "Ready".to_string(),
            visible_domains: HashMap::new(),
        }
    }
}

impl BingoTodoApp {
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            let _ = fs::write(&self.data_path, json);
        }
    }

    fn get_accent_color(&self, name: &str) -> egui::Color32 {
        self.data.domains.get(name)
            .map(|d| egui::Color32::from_rgb(d.accent[0], d.accent[1], d.accent[2]))
            .unwrap_or(egui::Color32::from_rgb(150, 150, 150))
    }

    fn completion_percent(&self, card: &DailyCard) -> f32 {
        if card.tasks.is_empty() { return 0.0; }
        (card.tasks.iter().filter(|t| t.completed).count() as f32 / card.tasks.len() as f32) * 100.0
    }

    fn sync_missing_tasks(&mut self, card_index: usize) {
        if let Some(card) = self.data.open_cards.get_mut(card_index) {
            if let Some(domain) = self.data.domains.get(&card.domain) {
                let existing: std::collections::HashSet<_> = card.tasks.iter().map(|t| t.id).collect();
                let mut added = 0;
                for t in &domain.template_tasks {
                    if !existing.contains(&t.id) {
                        card.tasks.push(t.clone());
                        added += 1;
                    }
                }
                self.status_message = format!("Synced {} missing task(s)", added);
                self.save();
            }
        }
    }

    fn archive_current_card(&mut self) {
        if let Some(idx) = self.data.current_card_index {
            if idx < self.data.open_cards.len() {
                let card = self.data.open_cards.remove(idx);
                self.data.archived_cards.push(card);
                self.data.current_card_index = if self.data.open_cards.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.save();
            }
        }
    }

    fn export_current_card_to_json(&mut self) {
        if let Some(idx) = self.data.current_card_index {
            if idx < self.data.open_cards.len() {
                if let Ok(json) = serde_json::to_string_pretty(&self.data.open_cards[idx]) {
                    let export_dir = self.data_path.parent().unwrap().join("exports");
                    if fs::create_dir_all(&export_dir).is_ok() {
                        let safe_name = self.data.open_cards[idx].domain.replace([' ', '/'], "_");
                        let filename = format!("{}_{}.json", safe_name, self.data.open_cards[idx].date);
                        let path = export_dir.join(filename);
                        if fs::write(&path, json).is_ok() {
                            self.status_message = format!("Exported to {}", path.display());
                        } else {
                            self.status_message = "Failed to write export file".to_string();
                        }
                    }
                }
            }
        }
    }

    fn reset_history(&mut self) {
        self.data.archived_cards.clear();
        self.save();
        self.status_message = "History cleared".to_string();
    }
}

impl eframe::App for BingoTodoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.theme == Theme::Dark { ctx.set_visuals(egui::Visuals::dark()); } else { ctx.set_visuals(egui::Visuals::light()); }
        ctx.set_pixels_per_point(self.ui_scale);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("bingo-todo");
                ui.separator();
                if ui.button("Daily View").clicked() { self.current_view = View::Daily; }
                if ui.button("Performance").clicked() { self.current_view = View::Performance; }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut scale = self.ui_scale;
                    let response = ui.add(egui::Slider::new(&mut scale, 0.85..=2.0).text("Scale").step_by(0.05));
                    if response.drag_stopped() || (response.lost_focus() && scale != self.ui_scale) {
                        self.ui_scale = scale.clamp(0.85, 2.0);
                    }

                    let label = if self.theme == Theme::Dark { "☀ Light" } else { "🌙 Dark" };
                    if ui.button(label).clicked() {
                        self.theme = if self.theme == Theme::Dark { Theme::Light } else { Theme::Dark };
                    }
                    ui.label(&self.status_message);
                });
            });

            ui.separator();

            match self.current_view {
                View::Daily => self.show_daily_view(ui),
                View::Performance => self.show_performance_view(ui),
            }
        });
    }
}

impl BingoTodoApp {
    fn show_daily_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(320.0);
                ui.heading("Domains");

                for name in self.data.domains.keys().cloned().collect::<Vec<_>>() {
                    let accent = self.get_accent_color(&name);
                    let button = egui::Button::new(egui::RichText::new(&name).color(accent).strong())
                        .fill(if self.theme == Theme::Dark {
                            egui::Color32::from_rgb(50, 50, 55)
                        } else {
                            egui::Color32::from_rgb(235, 235, 240)
                        })
                        .stroke(egui::Stroke::new(1.5, accent));

                    if ui.add(button).clicked() {
                        if let Some(d) = self.data.domains.get(&name) {
                            let today = Utc::now().format("%Y-%m-%d").to_string();
                            self.data.open_cards.push(DailyCard {
                                id: Uuid::new_v4(),
                                domain: name.clone(),
                                date: today,
                                tasks: d.template_tasks.clone(),
                                created_at: Utc::now(),
                                tags: vec![],
                            });
                            self.data.current_card_index = Some(self.data.open_cards.len() - 1);
                            self.save();
                        }
                    }
                }

                ui.separator();
                ui.heading(format!("Open Cards ({})", self.data.open_cards.len()));

                if self.data.open_cards.is_empty() {
                    ui.label("No active daily cards.");
                } else {
                    egui::ScrollArea::vertical()
                        .min_scrolled_height(500.0)
                        .min_scrolled_width(300.0)
                        .max_height(700.0)
                        .show(ui, |ui| {
                            for (i, card) in self.data.open_cards.iter().enumerate() {
				    let selected = self.data.current_card_index == Some(i);
				    let accent = self.get_accent_color(&card.domain);

				    let display_name = if !card.tags.is_empty() {
					card.tags.join(", ")
				    } else {
					card.domain.clone()
				    };

				    let label = format!("{} ({}) — {:.0}%", display_name, card.date, self.completion_percent(card));

				    // === Theme-aware colors ===
				    let (text_color, bg_color) = if selected {
					if self.theme == Theme::Dark {
					    // Dark mode: white text + subtle accent background
					    (egui::Color32::WHITE, accent.linear_multiply(0.35))
					} else {
					    // Light mode: dark text + soft accent background
					    (egui::Color32::from_rgb(30, 30, 30), accent.linear_multiply(0.15))
					}
				    } else {
					(accent, egui::Color32::TRANSPARENT)
				    };

				    let text = egui::RichText::new(label).color(text_color).strong();

				    let response = ui.selectable_label(selected, text);

				    // Draw custom background when selected
				    if selected && bg_color != egui::Color32::TRANSPARENT {
					ui.painter().rect_filled(
					    response.rect.expand(2.0),
					    5.0,
					    bg_color,
					);
				    }

				    if response.clicked() {
					self.data.current_card_index = Some(i);
				    }
				}
                        });
                }

                if ui.button("Archive all & close").clicked() {
                    self.data.archived_cards.append(&mut self.data.open_cards);
                    self.data.current_card_index = None;
                    self.save();
                }
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.set_min_width(540.0);
                ui.set_min_height(420.0);

                let idx = if let Some(i) = self.data.current_card_index {
                    if i < self.data.open_cards.len() { i } else { 0 }
                } else if !self.data.open_cards.is_empty() {
                    self.data.open_cards.len() - 1
                } else {
                    usize::MAX
                };

                if idx == usize::MAX || self.data.open_cards.is_empty() {
                    ui.centered_and_justified(|ui| { ui.label("Click a domain on the left to create a card."); });
                    return;
                }

                let card = self.data.open_cards[idx].clone();
                let accent = self.get_accent_color(&card.domain);
                let percent = self.completion_percent(&card);

                let frame_fill = if self.theme == Theme::Dark {
                    egui::Color32::from_rgb(40, 40, 45)
                } else {
                    egui::Color32::from_rgb(245, 245, 248)
                };

                egui::Frame::default()
                    .fill(frame_fill)
                    .stroke(egui::Stroke::new(2.0, accent))
                    .rounding(10.0)
                    .inner_margin(18.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.heading(egui::RichText::new(&card.domain).color(accent));
                                ui.label(format!("— {}", card.date));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(format!("{:.0}%", percent)).strong());
                                });
                            });

                            ui.add(egui::ProgressBar::new(percent / 100.0));
                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                ui.label("Tags:");
                                for (t_idx, tag) in self.data.open_cards[idx].tags.clone().iter().enumerate() {
                                    if ui.small_button(format!("× {}", tag)).clicked() {
                                        self.data.open_cards[idx].tags.remove(t_idx);
                                        self.save();
                                    }
                                }
                            });

                            ui.horizontal(|ui| {
                                let r = ui.add(egui::TextEdit::singleline(&mut self.new_tag_text).hint_text("New tag").desired_width(180.0));
                                if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Add Tag").clicked() {
                                    let tag = self.new_tag_text.trim().to_string();
                                    if !tag.is_empty() && !self.data.open_cards[idx].tags.contains(&tag) {
                                        self.data.open_cards[idx].tags.push(tag);
                                        self.new_tag_text.clear();
                                        self.save();
                                    }
                                }
                            });

                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                if ui.button("Sync missing tasks from template").clicked() {
                                    self.sync_missing_tasks(idx);
                                }
                                if ui.button("Archive this day").clicked() {
                                    self.archive_current_card();
                                }
                                if ui.button("Export JSON").clicked() {
                                    self.export_current_card_to_json();
                                }
                            });

                            ui.add_space(8.0);

                            ui.horizontal(|ui| {
                                let r = ui.add(egui::TextEdit::singleline(&mut self.new_task_text).hint_text("New task...").desired_width(380.0));
                                if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Add").clicked() {
                                    if !self.new_task_text.trim().is_empty() {
                                        self.data.open_cards[idx].tasks.push(Task {
                                            id: Uuid::new_v4(),
                                            description: self.new_task_text.trim().to_string(),
                                            completed: false,
                                            notes: None,
                                            tags: vec![],
                                        });
                                        self.new_task_text.clear();
                                        self.save();
                                    }
                                }
                            });

                            ui.add_space(8.0);

                            let mut to_toggle = None;
                            let mut to_delete = None;

                            for (i, task) in self.data.open_cards[idx].tasks.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let mut done = task.completed;
                                    if ui.checkbox(&mut done, "").clicked() { to_toggle = Some(i); }
                                    ui.label(&task.description);
                                    if ui.small_button("✕").clicked() { to_delete = Some(i); }
                                });
                            }

                            if let Some(i) = to_toggle {
                                self.data.open_cards[idx].tasks[i].completed = !self.data.open_cards[idx].tasks[i].completed;
                                self.save();
                            }
                            if let Some(d) = to_delete {
                                self.data.open_cards[idx].tasks.remove(d);
                                self.save();
                            }
                        });
                    });
            });
        });
    }

    fn show_performance_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Performance");
        ui.separator();

        let all_cards: Vec<&DailyCard> = self.data.open_cards.iter()
            .chain(self.data.archived_cards.iter())
            .collect();

        if all_cards.is_empty() {
            ui.label("No completed days yet.");
            return;
        }

        let total_tasks: usize = all_cards.iter().map(|c| c.tasks.len()).sum();
        let total_done: usize = all_cards.iter().map(|c| c.tasks.iter().filter(|t| t.completed).count()).sum();
        let overall_pct = if total_tasks > 0 { (total_done as f32 / total_tasks as f32) * 100.0 } else { 0.0 };

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Overall Completion (all time)");
                ui.heading(format!("{:.1}%", overall_pct));
                ui.label(format!("{} days tracked", all_cards.len()));
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.label("Per Domain (all time)");
                for (name, _) in &self.data.domains {
                    let cards: Vec<_> = all_cards.iter().filter(|c| c.domain == *name).collect();
                    if !cards.is_empty() {
                        let avg = cards.iter().map(|c| self.completion_percent(c)).sum::<f32>() / cards.len() as f32;
                        ui.label(format!("{}: {:.0}% ({} days)", name, avg, cards.len()));
                    }
                }
            });
        });

        ui.add_space(16.0);

        // === Time Series Graph ===
        ui.heading("Completion % Over Time");

        let mut domain_series: HashMap<String, Vec<(f64, f32)>> = HashMap::new();

        for card in &all_cards {
            let timestamp = card.created_at.timestamp() as f64;
            let pct = self.completion_percent(card);
            domain_series.entry(card.domain.clone()).or_default().push((timestamp, pct));
        }

        for series in domain_series.values_mut() {
            series.sort_by_key(|(t, _)| *t as i64);
        }

        if self.visible_domains.is_empty() {
            for name in self.data.domains.keys() {
                self.visible_domains.insert(name.clone(), true);
            }
        }

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
		    ui.label("Show on graph:");
		    let domain_names: Vec<String> = self.visible_domains.keys().cloned().collect();
		    for name in domain_names {
			let accent = self.get_accent_color(&name);           // ← Get accent first
			if let Some(visible) = self.visible_domains.get_mut(&name) {
			    ui.checkbox(visible, egui::RichText::new(&name).color(accent));
			}
		    }
		});

            ui.separator();

            egui_plot::Plot::new("completion_over_time")
            .width(ui.available_width())   // Use remaining width
            .height(640.0)
	    .legend(egui_plot::Legend::default())
	    .x_axis_label("Date + Time Added")
	    .y_axis_label("Completion %")
	    .x_axis_formatter(|mark, _range, _| {
		let x = mark.value;

		chrono::DateTime::<Utc>::from_timestamp(x as i64, 0)
		    .map(|dt| dt.format("%b %d %H:%M").to_string())
		    .unwrap_or_else(|| format!("{:.0}", x))
	    })
	    .show(ui, |plot_ui| {
		for (domain_name, points) in &domain_series {
		    if *self.visible_domains.get(domain_name).unwrap_or(&true) {
		        let accent = self.get_accent_color(domain_name);

		        let plot_points: Vec<[f64; 2]> = points
		            .iter()
		            .map(|(t, pct)| [*t, *pct as f64])
		            .collect();

		        if !plot_points.is_empty() {
		            plot_ui.line(
		                egui_plot::Line::new(plot_points)
		                    .name(domain_name.clone())
		                    .color(accent)
		                    .width(2.5)
		            );
		        }
		    }
		}
	    });
        });

        ui.add_space(16.0);
        if ui.button("Reset / Clear All History").clicked() {
            self.reset_history();
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 650.0])
            .with_title("bingo-todo"),
        ..Default::default()
    };

    eframe::run_native(
        "bingo-todo",
        options,
        Box::new(|_cc| Box::new(BingoTodoApp::default()) as Box<dyn eframe::App>),
    )
}