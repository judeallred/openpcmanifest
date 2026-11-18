//! The graphical interface, built with `eframe`/`egui`.
//!
//! Layout follows Speccy: sections on the left, the selected section's tree
//! on the right. Collection runs on a background thread and reports progress
//! group by group, so sections appear as soon as their data arrives.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};

use crate::build::build;
use crate::collect::{GROUPS, Snapshot, collect_with_progress};
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
        Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))),
    )
}

/// A message from the collection thread.
enum Msg {
    /// The snapshot so far and the groups that have finished.
    Progress(Snapshot, Vec<&'static str>),
    /// Collection finished.
    Done(Result<Snapshot, String>),
}

/// Where the data on screen came from.
enum State {
    /// Collection is running; the groups listed have finished.
    Loading(Receiver<Msg>, Vec<&'static str>),
    /// A complete report is ready.
    Ready,
    /// Collection failed.
    Failed(String),
}

struct App {
    ctx: egui::Context,
    state: State,
    snapshot: Option<Snapshot>,
    report: Option<Report>,
    /// When set, the window shows a comparison instead of the report.
    comparison: Option<Report>,
    selected: &'static str,
    search: String,
    hide_private: bool,
    redact_exports: bool,
    message: Option<(String, Instant)>,
    show_about: bool,
}

/// A friendly description of what each collection group covers.
const fn group_label(group: &str) -> &'static str {
    match group.as_bytes() {
        b"core" => "system, CPU, RAM and motherboard",
        b"graphics" => "graphics",
        b"storage" => "drives",
        b"network" => "network",
        b"devices" => "devices",
        b"security" => "security and updates",
        b"drivers" => "drivers",
        _ => "other details",
    }
}

impl App {
    fn new(ctx: egui::Context) -> Self {
        let state = start_collection(&ctx);
        Self {
            ctx,
            state,
            snapshot: None,
            report: None,
            comparison: None,
            selected: "summary",
            search: String::new(),
            hide_private: false,
            redact_exports: true,
            message: None,
            show_about: false,
        }
    }

    fn show_snapshot(&mut self, snap: Snapshot) {
        self.report = Some(build(&snap));
        self.snapshot = Some(snap);
    }

    fn set_snapshot(&mut self, snap: Snapshot) {
        self.show_snapshot(snap);
        self.comparison = None;
        self.state = State::Ready;
    }

    fn notify(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), Instant::now()));
    }

    fn poll(&mut self) {
        loop {
            let State::Loading(rx, _) = &self.state else {
                return;
            };
            match rx.try_recv() {
                Ok(Msg::Progress(snap, done)) => {
                    self.show_snapshot(snap);
                    if let State::Loading(_, finished) = &mut self.state {
                        *finished = done;
                    }
                }
                Ok(Msg::Done(Ok(snap))) => self.set_snapshot(snap),
                Ok(Msg::Done(Err(e))) => self.state = State::Failed(e),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.state = State::Failed("The collector stopped unexpectedly.".to_owned());
                }
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
                self.selected = "summary";
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
                self.selected = "changes";
                self.notify(format!(
                    "{} changes since {}",
                    changes.len(),
                    path.display()
                ));
            }
            Err(e) => self.notify(format!("Could not open: {e}")),
        }
    }

    fn copy_summary(&mut self, format: Format) {
        if let Some(report) = self.exportable() {
            self.ctx.copy_text(rig_summary(&report, format));
            self.notify(format!("Copied the summary as {}", format.name()));
        }
    }

    fn refresh(&mut self) {
        self.state = start_collection(&self.ctx);
        self.comparison = None;
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let ready = matches!(self.state, State::Ready);
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open snapshot\u{2026}").clicked() {
                    ui.close();
                    self.open_snapshot();
                }
                if ui
                    .add_enabled(ready, egui::Button::new("Save snapshot\u{2026}"))
                    .clicked()
                {
                    ui.close();
                    self.save_snapshot();
                }
                ui.add_enabled_ui(self.report.is_some(), |ui| {
                    ui.menu_button("Save report as", |ui| {
                        for f in Format::ALL {
                            if ui.button(f.name()).clicked() {
                                ui.close();
                                self.save_report(f);
                            }
                        }
                    });
                });
                if ui
                    .add_enabled(ready, egui::Button::new("Compare with snapshot\u{2026}"))
                    .clicked()
                {
                    ui.close();
                    self.compare_with_snapshot();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
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
                    ui.close();
                    open_url(crate::WIKI_URL);
                }
                if ui.button("Report a bug or request a feature").clicked() {
                    ui.close();
                    open_url(crate::ISSUES_URL);
                }
                if ui.button("Project home").clicked() {
                    ui.close();
                    open_url(crate::REPOSITORY);
                }
                ui.separator();
                if ui.button("About").clicked() {
                    ui.close();
                    self.show_about = true;
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let loading = matches!(self.state, State::Loading(..));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!loading, egui::Button::new("\u{27f3} Refresh"))
                .clicked()
            {
                self.refresh();
            }
            ui.add_enabled_ui(self.report.is_some(), |ui| {
                ui.menu_button("Copy summary", |ui| {
                    for f in [Format::Markdown, Format::BbCode, Format::Text] {
                        if ui.button(f.name()).clicked() {
                            ui.close();
                            self.copy_summary(f);
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

    /// While loading, a line saying what is still on its way.
    fn loading_banner(&self, ui: &mut egui::Ui) {
        let State::Loading(_, done) = &self.state else {
            return;
        };
        let pending: Vec<&str> = GROUPS
            .iter()
            .filter(|g| !done.contains(g))
            .map(|g| group_label(g))
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.spinner();
            ui.label(if pending.is_empty() {
                "Finishing up\u{2026}".to_owned()
            } else {
                format!("Still reading: {}", pending.join(", "))
            });
        });
        ui.add_space(6.0);
    }

    fn about_window(&mut self) {
        let mut open = self.show_about;
        egui::Window::new("About")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(&self.ctx, |ui| {
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

    fn sections_panel(&mut self, ui: &mut egui::Ui, report: Option<&Report>, searching: bool) {
        let Some(r) = report else { return };
        if self.comparison.is_some() && ui.button("\u{2190} Back to full report").clicked() {
            self.comparison = None;
            self.selected = "summary";
        }
        ui.add_space(4.0);
        for s in &r.sections {
            ui.add_enabled_ui(!searching, |ui| {
                ui.selectable_value(&mut self.selected, s.id, s.title);
            });
        }
        if let State::Loading(_, done) = &self.state {
            ui.add_space(8.0);
            for g in GROUPS.iter().filter(|g| !done.contains(g)) {
                ui.label(
                    RichText::new(format!("Loading {}\u{2026}", group_label(g)))
                        .weak()
                        .italics(),
                );
            }
        }
    }

    fn main_panel(&self, ui: &mut egui::Ui, report: Option<&Report>, searching: bool) {
        if let State::Failed(e) = &self.state {
            ui.heading("Could not read this PC");
            ui.label(e.as_str());
            ui.label("Open a saved snapshot from the File menu, or report the problem from the Help menu.");
            return;
        }
        self.loading_banner(ui);
        let Some(r) = report else { return };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if searching {
                    let found = r.filtered(&self.search, self.hide_private);
                    if found.sections.is_empty() {
                        ui.label("No matches.");
                    }
                    for s in &found.sections {
                        section_view(ui, s, true);
                        ui.add_space(12.0);
                    }
                } else if let Some(s) = r.section(self.selected).or_else(|| r.sections.first()) {
                    section_view(ui, s, false);
                }
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.show_about {
            self.about_window();
        }
        let report = self.current();
        let searching = !self.search.trim().is_empty();
        egui::Panel::left("sections")
            .resizable(false)
            .exact_size(210.0)
            .show(ui, |ui| self.sections_panel(ui, report.as_ref(), searching));
        egui::CentralPanel::default()
            .show(ui, |ui| self.main_panel(ui, report.as_ref(), searching));
        if self
            .message
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() < Duration::from_secs(6))
        {
            self.ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

/// Starts collection on a background thread. Each finished group is sent to
/// the window, which repaints to show it.
fn start_collection(ctx: &egui::Context) -> State {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = collect_with_progress(|snap, done| {
            // The receiver may be gone if the window closed; nothing to do then.
            let _ = tx.send(Msg::Progress(snap.clone(), done.to_vec()));
            ctx.request_repaint();
        });
        let _ = tx.send(Msg::Done(result.map_err(|e| e.to_string())));
        ctx.request_repaint();
    });
    State::Loading(rx, Vec::new())
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
                nodes_view(ui, &n.children, &child_path, depth + 1, open_all);
            });
    }
}
