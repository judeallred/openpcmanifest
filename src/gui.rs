//! The graphical interface, built with `eframe`/`egui`.
//!
//! Layout follows Speccy: sections on the left, the selected section's tree
//! on the right. Collection runs on a background thread so the window opens
//! immediately.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};

use crate::build::build;
use crate::collect::{Snapshot, collect};
use crate::diff;
use crate::model::{Node, Report, Section, Status};
use crate::render::{Format, render, rig_summary};

/// Opens the main window and blocks until it is closed.
///
/// # Errors
///
/// Returns an error if the window or graphics context cannot be created.
pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(crate::build_string())
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        crate::NAME,
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

/// Where the data on screen came from.
enum State {
    /// Collection is running on a background thread.
    Loading(Receiver<Result<Snapshot, String>>),
    /// A report is ready.
    Ready,
    /// Collection failed.
    Failed(String),
}

struct App {
    state: State,
    snapshot: Option<Snapshot>,
    report: Option<Report>,
    /// When set, the window shows a comparison instead of the report.
    comparison: Option<Report>,
    selected: usize,
    search: String,
    hide_private: bool,
    redact_exports: bool,
    message: Option<(String, Instant)>,
    show_about: bool,
}

impl App {
    fn new() -> Self {
        Self {
            state: start_collection(),
            snapshot: None,
            report: None,
            comparison: None,
            selected: 0,
            search: String::new(),
            hide_private: false,
            redact_exports: true,
            message: None,
            show_about: false,
        }
    }

    fn set_snapshot(&mut self, snap: Snapshot) {
        self.report = Some(build(&snap));
        self.snapshot = Some(snap);
        self.comparison = None;
        self.selected = 0;
        self.state = State::Ready;
    }

    fn notify(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), Instant::now()));
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let State::Loading(rx) = &self.state else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(snap)) => self.set_snapshot(snap),
            Ok(Err(e)) => self.state = State::Failed(e),
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => {
                self.state = State::Failed("The collector stopped unexpectedly.".to_owned());
            }
        }
    }

    /// The report currently on screen, before search filtering.
    fn current(&self) -> Option<Report> {
        let base = self.comparison.as_ref().or(self.report.as_ref())?;
        Some(if self.hide_private {
            base.redacted()
        } else {
            base.clone()
        })
    }

    /// The report as it should be exported.
    fn exportable(&self) -> Option<Report> {
        let base = self.comparison.as_ref().or(self.report.as_ref())?;
        Some(if self.redact_exports {
            base.redacted()
        } else {
            base.clone()
        })
    }

    fn save_report(&mut self, format: Format) {
        let Some(report) = self.exportable() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("pc-report.{}", format.extension()))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, render(&report, format)) {
            Ok(()) => self.notify(format!("Saved {}", path.display())),
            Err(e) => self.notify(format!("Could not save: {e}")),
        }
    }

    fn save_snapshot(&mut self) {
        let Some(snap) = &self.snapshot else { return };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Raw snapshot", &["json"])
            .set_file_name("pc-snapshot.json")
            .save_file()
        else {
            return;
        };
        let result = snap.save(&path);
        match result {
            Ok(()) => self.notify(format!("Saved snapshot to {}", path.display())),
            Err(e) => self.notify(format!("Could not save: {e}")),
        }
    }

    fn open_snapshot(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Raw snapshot", &["json"])
            .pick_file()
        else {
            return;
        };
        match Snapshot::load(&path) {
            Ok(snap) => {
                self.set_snapshot(snap);
                self.notify(format!("Opened {}", path.display()));
            }
            Err(e) => self.notify(format!("Could not open: {e}")),
        }
    }

    fn compare_with_snapshot(&mut self) {
        let Some(current) = self.report.clone() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Raw snapshot", &["json"])
            .pick_file()
        else {
            return;
        };
        match Snapshot::load(&path) {
            Ok(old) => {
                let old = build(&old);
                let changes = diff::compare(&old, &current, true);
                self.comparison = Some(diff::to_report(&changes, &old, &current));
                self.selected = 0;
                self.notify(format!(
                    "{} changes since {}",
                    changes.len(),
                    path.display()
                ));
            }
            Err(e) => self.notify(format!("Could not open: {e}")),
        }
    }

    fn copy_summary(&mut self, ctx: &egui::Context, format: Format) {
        if let Some(report) = self.exportable() {
            ctx.copy_text(rig_summary(&report, format));
            self.notify(format!("Copied the summary as {}", format.name()));
        }
    }

    fn menu_bar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let ready = matches!(self.state, State::Ready);
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open snapshot\u{2026}").clicked() {
                    ui.close_menu();
                    self.open_snapshot();
                }
                if ui
                    .add_enabled(ready, egui::Button::new("Save snapshot\u{2026}"))
                    .clicked()
                {
                    ui.close_menu();
                    self.save_snapshot();
                }
                ui.add_enabled_ui(ready, |ui| {
                    ui.menu_button("Save report as", |ui| {
                        for f in Format::ALL {
                            if ui.button(f.name()).clicked() {
                                ui.close_menu();
                                self.save_report(f);
                            }
                        }
                    });
                });
                if ui
                    .add_enabled(ready, egui::Button::new("Compare with snapshot\u{2026}"))
                    .clicked()
                {
                    ui.close_menu();
                    self.compare_with_snapshot();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.hide_private, "Hide private details on screen");
                ui.checkbox(
                    &mut self.redact_exports,
                    "Hide private details in exports and copies",
                );
            });
            ui.menu_button("Help", |ui| {
                if ui.button("User guide (wiki)").clicked() {
                    ui.close_menu();
                    open_url(crate::WIKI_URL);
                }
                if ui.button("Report a bug or request a feature").clicked() {
                    ui.close_menu();
                    open_url(crate::ISSUES_URL);
                }
                if ui.button("Project home").clicked() {
                    ui.close_menu();
                    open_url(crate::REPOSITORY);
                }
                ui.separator();
                if ui.button("About").clicked() {
                    ui.close_menu();
                    self.show_about = true;
                }
            });
        });
    }

    fn toolbar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let ready = matches!(self.state, State::Ready);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !matches!(self.state, State::Loading(_)),
                    egui::Button::new("\u{27f3} Refresh"),
                )
                .clicked()
            {
                self.state = start_collection();
            }
            ui.add_enabled_ui(ready, |ui| {
                ui.menu_button("Copy summary", |ui| {
                    for f in [Format::Markdown, Format::BbCode, Format::Text] {
                        if ui.button(f.name()).clicked() {
                            ui.close_menu();
                            self.copy_summary(ctx, f);
                        }
                    }
                });
            });
            ui.checkbox(&mut self.hide_private, "Hide private details");
            ui.separator();
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search all sections")
                    .desired_width(240.0),
            );
            if !self.search.is_empty() && ui.button("Clear").clicked() {
                self.search.clear();
            }
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if let Some((text, at)) = &self.message
                && at.elapsed() < Duration::from_secs(6)
            {
                ui.label(RichText::new(text).strong());
                ui.separator();
            }
            if let Some(report) = &self.report {
                for note in &report.notes {
                    ui.label(RichText::new(note).color(status_color(Status::Caution)));
                }
                if let Some(at) = &report.collected_at {
                    ui.label(RichText::new(format!("Collected {at}")).weak());
                }
            }
            ui.label(RichText::new(crate::build_string()).weak());
        });
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new("About")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading(crate::NAME);
                ui.label(crate::build_string());
                ui.label("A free, open-source hardware inventory for Windows. MIT licensed.");
                ui.label("No telemetry, no network access, no bundled software.");
                if ui.link(crate::REPOSITORY).clicked() {
                    open_url(crate::REPOSITORY);
                }
            });
        self.show_about = open;
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.menu_bar(ctx, ui);
            self.toolbar(ctx, ui);
            ui.add_space(2.0);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        if self.show_about {
            self.about_window(ctx);
        }
        let report = self.current();
        let searching = !self.search.trim().is_empty();
        egui::SidePanel::left("sections")
            .resizable(false)
            .exact_width(200.0)
            .show(ctx, |ui| {
                if let Some(r) = &report {
                    if self.comparison.is_some()
                        && ui.button("\u{2190} Back to full report").clicked()
                    {
                        self.comparison = None;
                        self.selected = 0;
                    }
                    ui.add_space(4.0);
                    for (i, s) in r.sections.iter().enumerate() {
                        ui.add_enabled_ui(!searching, |ui| {
                            ui.selectable_value(&mut self.selected, i, s.title);
                        });
                    }
                }
            });
        egui::CentralPanel::default().show(ctx, |ui| match (&self.state, &report) {
            (State::Loading(_), _) => {
                ui.centered_and_justified(|ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading your hardware\u{2026} this takes a few seconds.");
                    });
                });
            }
            (State::Failed(e), _) => {
                ui.heading("Could not read this PC");
                ui.label(e.as_str());
                ui.label("Open a saved snapshot from the File menu, or report the problem from the Help menu.");
            }
            (State::Ready, Some(r)) => {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if searching {
                        let found = r.filtered(&self.search, self.hide_private);
                        if found.sections.is_empty() {
                            ui.label("No matches.");
                        }
                        for s in &found.sections {
                            section_view(ui, s, true);
                            ui.add_space(12.0);
                        }
                    } else if let Some(s) = r.sections.get(self.selected).or_else(|| r.sections.first()) {
                        section_view(ui, s, false);
                    }
                });
            }
            (State::Ready, None) => {
                ui.label("Nothing to show.");
            }
        });
    }
}

fn start_collection() -> State {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // The receiver may be gone if the window closed; nothing to do then.
        let _ = tx.send(collect().map_err(|e| e.to_string()));
    });
    State::Loading(rx)
}

/// Opens a web page in the default browser. This is the only thing the app
/// does that can lead to network traffic, and only when the user clicks.
fn open_url(url: &str) {
    #[cfg(windows)]
    let result = std::process::Command::new("explorer.exe").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    // Nothing useful to do if no browser is registered.
    drop(result);
}

const fn status_color(status: Status) -> Color32 {
    match status {
        Status::Good => Color32::from_rgb(63, 185, 80),
        Status::Caution => Color32::from_rgb(210, 153, 34),
        Status::Bad => Color32::from_rgb(248, 81, 73),
        Status::Note | Status::Normal => Color32::GRAY,
    }
}

fn value_text(n: &Node) -> RichText {
    let text = RichText::new(n.value.as_deref().unwrap_or(""));
    match n.status {
        Status::Normal => text,
        Status::Note => text.italics().color(status_color(Status::Note)),
        other => text.strong().color(status_color(other)),
    }
}

fn section_view(ui: &mut egui::Ui, s: &Section, open_all: bool) {
    ui.heading(s.title);
    ui.add_space(4.0);
    nodes_view(ui, &s.nodes, s.id, 0, open_all);
}

fn nodes_view(ui: &mut egui::Ui, nodes: &[Node], path: &str, depth: usize, open_all: bool) {
    let leaves: Vec<&Node> = nodes.iter().filter(|n| n.children.is_empty()).collect();
    if !leaves.is_empty() {
        egui::Grid::new(format!("{path}/grid"))
            .num_columns(2)
            .striped(true)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                for n in leaves {
                    ui.add(egui::Label::new(RichText::new(&n.label).weak()).selectable(true));
                    ui.add(egui::Label::new(value_text(n)).selectable(true).wrap());
                    ui.end_row();
                }
            });
    }
    for (i, n) in nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.children.is_empty())
    {
        let child_path = format!("{path}/{}#{i}", n.key);
        let mut title = RichText::new(&n.label).strong();
        if let Some(v) = &n.value {
            title = RichText::new(format!("{}  \u{2014}  {v}", n.label)).strong();
            if n.status != Status::Normal {
                title = title.color(status_color(n.status));
            }
        }
        egui::CollapsingHeader::new(title)
            .id_salt(&child_path)
            .default_open(open_all || depth < 1)
            .show(ui, |ui| {
                nodes_view(ui, &n.children, &child_path, depth + 1, open_all)
            });
    }
}
