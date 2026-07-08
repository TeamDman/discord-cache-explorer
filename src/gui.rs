use crate::cache::{CacheEntry, CacheKind};
use chrono::{DateTime, Local};
use eframe::egui::{self, TextureHandle, TextureOptions};
use egui_extras::{Column, TableBuilder};
use eyre::eyre;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
enum Pane {
    Files,
    Preview,
    Details,
}

impl Pane {
    fn title(&self) -> &'static str {
        match self {
            Self::Files => "Cache Files",
            Self::Preview => "Preview",
            Self::Details => "Details",
        }
    }
}

struct App {
    cache_dir: PathBuf,
    entries: Vec<CacheEntry>,
    selected: Option<usize>,
    sort_key: SortKey,
    sort_direction: SortDirection,
    type_filter: TypeFilter,
    error: Option<String>,
    tree: egui_tiles::Tree<Pane>,
    preview_texture: Option<TextureHandle>,
    preview_texture_path: Option<PathBuf>,
    preview_error: Option<String>,
}

/// # Errors
///
/// Returns an error if the GUI fails to start.
pub fn run_gui(cache_dir_override: Option<PathBuf>) -> eyre::Result<()> {
    let cache_dir = match cache_dir_override {
        Some(path) => path,
        None => crate::cache::default_cache_dir()?,
    };

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Discord Cache Explorer",
        native_options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, cache_dir)))),
    )
    .map_err(|error| eyre!("failed to run eframe: {error}"))
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, cache_dir: PathBuf) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut app = Self {
            cache_dir,
            entries: Vec::new(),
            selected: None,
            sort_key: SortKey::Modified,
            sort_direction: SortDirection::Descending,
            type_filter: TypeFilter::default(),
            error: None,
            tree: create_tree(),
            preview_texture: None,
            preview_texture_path: None,
            preview_error: None,
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        match crate::cache::scan_cache_dir(&self.cache_dir) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
                self.sort_entries_preserving_selection();
            }
            Err(error) => {
                self.entries.clear();
                self.selected = None;
                self.error = Some(error.to_string());
            }
        }
        self.preview_texture = None;
        self.preview_texture_path = None;
        self.preview_error = None;
    }

    fn selected_entry(&self) -> Option<&CacheEntry> {
        self.selected.and_then(|index| self.entries.get(index))
    }

    fn set_sort(&mut self, sort_key: SortKey) {
        if self.sort_key == sort_key {
            self.sort_direction = self.sort_direction.reversed();
        } else {
            self.sort_key = sort_key;
            self.sort_direction = sort_key.default_direction();
        }
        self.sort_entries_preserving_selection();
    }

    fn sort_entries_preserving_selection(&mut self) {
        let selected_path = self
            .selected
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.path.clone());

        let sort_key = self.sort_key;
        let sort_direction = self.sort_direction;
        self.entries.sort_by(|a, b| {
            let ordering = sort_key.compare(a, b);
            match sort_direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            }
        });

        self.selected =
            selected_path.and_then(|path| self.entries.iter().position(|entry| entry.path == path));
    }

    fn clear_selection_if_filtered_out(&mut self) {
        if self
            .selected_entry()
            .is_some_and(|entry| !self.type_filter.matches(entry.kind))
        {
            self.selected = None;
            self.preview_texture = None;
            self.preview_texture_path = None;
            self.preview_error = None;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TypeFilter {
    image: bool,
    video: bool,
    other: bool,
}

impl Default for TypeFilter {
    fn default() -> Self {
        Self {
            image: true,
            video: true,
            other: true,
        }
    }
}

impl TypeFilter {
    fn matches(self, kind: CacheKind) -> bool {
        match kind {
            CacheKind::Image => self.image,
            CacheKind::Video => self.video,
            CacheKind::Other => self.other,
        }
    }

    fn active_count(self) -> usize {
        usize::from(self.image) + usize::from(self.video) + usize::from(self.other)
    }

    fn label(self) -> &'static str {
        match self.active_count() {
            0 => "none",
            3 => "all",
            _ => "filtered",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SortKey {
    Name,
    Modified,
    Kind,
    Size,
}

impl SortKey {
    fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Modified => "Modified",
            Self::Kind => "Type",
            Self::Size => "Size",
        }
    }

    fn default_direction(self) -> SortDirection {
        match self {
            Self::Name | Self::Kind => SortDirection::Ascending,
            Self::Modified | Self::Size => SortDirection::Descending,
        }
    }

    fn compare(self, a: &CacheEntry, b: &CacheEntry) -> Ordering {
        match self {
            Self::Name => naturalish_cmp(&a.file_name, &b.file_name),
            Self::Modified => compare_modified(a.modified, b.modified),
            Self::Kind => a
                .kind
                .label()
                .cmp(b.kind.label())
                .then_with(|| naturalish_cmp(&a.file_name, &b.file_name)),
            Self::Size => a
                .len
                .cmp(&b.len)
                .then_with(|| naturalish_cmp(&a.file_name, &b.file_name)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    fn arrow(self) -> &'static str {
        match self {
            Self::Ascending => "up",
            Self::Descending => "down",
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                if ui.button("Refresh").clicked() {
                    self.reload();
                }
                ui.label(self.cache_dir.display().to_string());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_switch(ui);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut tree = std::mem::replace(&mut self.tree, create_tree());
            let mut behavior = Behavior { app: self };
            tree.ui(&mut behavior, ui);
            self.tree = tree;
        });
    }
}

struct Behavior<'a> {
    app: &'a mut App,
}

impl egui_tiles::Behavior<Pane> for Behavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::Files => draw_files(ui, self.app),
            Pane::Preview => draw_preview(ui, self.app),
            Pane::Details => draw_details(ui, self.app),
        }
        egui_tiles::UiResponse::None
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}

fn create_tree() -> egui_tiles::Tree<Pane> {
    let mut tiles = egui_tiles::Tiles::default();
    let files = tiles.insert_pane(Pane::Files);
    let preview = tiles.insert_pane(Pane::Preview);
    let details = tiles.insert_pane(Pane::Details);
    let right = tiles.insert_vertical_tile(vec![preview, details]);
    let root = tiles.insert_horizontal_tile(vec![files, right]);
    egui_tiles::Tree::new("discord_cache_explorer_tree", root, tiles)
}

fn draw_files(ui: &mut egui::Ui, app: &mut App) {
    if let Some(error) = &app.error {
        ui.colored_label(egui::Color32::RED, error);
        return;
    }

    let filtered_indices: Vec<_> = app
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| app.type_filter.matches(entry.kind).then_some(index))
        .collect();

    ui.horizontal(|ui| {
        ui.strong(format!(
            "{} / {} files",
            filtered_indices.len(),
            app.entries.len()
        ));
        ui.label(format!(
            "Sorted by {} {}",
            app.sort_key.label(),
            app.sort_direction.arrow()
        ));
        ui.label(format!("Types: {}", app.type_filter.label()));
    });
    ui.separator();

    let mut pending_sort = None;
    let mut pending_selection = None;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false])
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(140.0))
        .column(Column::auto().at_least(84.0))
        .column(Column::auto().at_least(128.0))
        .column(Column::auto().at_least(72.0))
        .header(22.0, |mut header| {
            header.col(|ui| header_button(ui, app, SortKey::Name, &mut pending_sort));
            header.col(|ui| type_header_button(ui, app, &mut pending_sort));
            header.col(|ui| header_button(ui, app, SortKey::Modified, &mut pending_sort));
            header.col(|ui| header_button(ui, app, SortKey::Size, &mut pending_sort));
        })
        .body(|body| {
            body.rows(22.0, filtered_indices.len(), |mut row| {
                let index = filtered_indices[row.index()];
                let entry = &app.entries[index];
                row.col(|ui| {
                    let response = ui
                        .add_sized(
                            [ui.available_width(), 20.0],
                            egui::Button::selectable(app.selected == Some(index), &entry.file_name),
                        )
                        .on_hover_text(entry.path.display().to_string());
                    if response.clicked() {
                        pending_selection = Some(index);
                    }
                });
                row.col(|ui| {
                    ui.label(format!("{} {}", kind_icon(entry.kind), entry.kind.label()));
                });
                row.col(|ui| {
                    ui.label(format_modified(entry.modified));
                });
                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format_bytes(entry.len));
                    });
                });
            });
        });

    if let Some(sort_key) = pending_sort {
        app.set_sort(sort_key);
    }

    if let Some(index) = pending_selection {
        app.selected = Some(index);
        app.preview_texture = None;
        app.preview_texture_path = None;
        app.preview_error = None;
    }
}

fn header_button(
    ui: &mut egui::Ui,
    app: &App,
    sort_key: SortKey,
    pending_sort: &mut Option<SortKey>,
) {
    let label = if app.sort_key == sort_key {
        format!("{} {}", sort_key.label(), app.sort_direction.arrow())
    } else {
        sort_key.label().to_string()
    };
    if ui.button(label).clicked() {
        *pending_sort = Some(sort_key);
    }
}

fn type_header_button(ui: &mut egui::Ui, app: &mut App, pending_sort: &mut Option<SortKey>) {
    let label = if app.sort_key == SortKey::Kind {
        format!("{} {}", SortKey::Kind.label(), app.sort_direction.arrow())
    } else {
        SortKey::Kind.label().to_string()
    };

    let response = ui
        .button(label)
        .on_hover_text("Left-click to sort. Right-click to filter.");
    if response.clicked() {
        *pending_sort = Some(SortKey::Kind);
    }

    response.context_menu(|ui| {
        ui.strong("Inferred type");
        ui.separator();

        let mut changed = false;
        changed |= ui.checkbox(&mut app.type_filter.image, "Image").changed();
        changed |= ui.checkbox(&mut app.type_filter.video, "Video").changed();
        changed |= ui.checkbox(&mut app.type_filter.other, "Other").changed();

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("All").clicked() {
                app.type_filter = TypeFilter::default();
                changed = true;
            }
            if ui.button("None").clicked() {
                app.type_filter = TypeFilter {
                    image: false,
                    video: false,
                    other: false,
                };
                changed = true;
            }
        });

        if changed {
            app.clear_selection_if_filtered_out();
        }
    });
}

fn draw_preview(ui: &mut egui::Ui, app: &mut App) {
    let Some(entry) = app.selected_entry().cloned() else {
        ui.centered_and_justified(|ui| {
            ui.label("Select a cache file to preview it.");
        });
        return;
    };

    ui.horizontal(|ui| {
        ui.strong(&entry.file_name);
        ui.label(entry.kind.label());
    });
    ui.separator();

    match entry.kind {
        CacheKind::Image => draw_image_preview(ui, app, &entry.path),
        CacheKind::Video => {
            draw_video_preview(ui, app, &entry.path);
        }
        CacheKind::Other => {
            ui.label("No preview available yet for this file type.");
            ui.monospace(entry.path.display().to_string());
        }
    }
}

fn draw_image_preview(ui: &mut egui::Ui, app: &mut App, path: &Path) {
    let needs_load = app.preview_texture_path.as_deref() != Some(path);
    if needs_load {
        match load_texture(ui, path) {
            Ok(texture) => {
                app.preview_texture = Some(texture);
                app.preview_error = None;
            }
            Err(error) => {
                app.preview_texture = None;
                app.preview_error = Some(error.to_string());
            }
        }
        app.preview_texture_path = Some(path.to_path_buf());
    }

    if let Some(texture) = &app.preview_texture {
        let available = ui.available_size();
        let texture_size = texture.size_vec2();
        let scale = (available.x / texture_size.x)
            .min(available.y / texture_size.y)
            .min(1.0)
            .max(0.05);
        ui.image((texture.id(), texture_size * scale));
    } else {
        ui.label("Image loader could not decode this cache file.");
        if let Some(error) = &app.preview_error {
            ui.monospace(error);
        }
    }
}

fn load_texture(ui: &egui::Ui, path: &Path) -> eyre::Result<TextureHandle> {
    let image = crate::cache::decode_image(path)?;
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let pixels = rgba.as_flat_samples();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    Ok(ui.ctx().load_texture(
        format!("preview_{}", path.display()),
        color_image,
        TextureOptions::default(),
    ))
}

fn draw_video_preview(ui: &mut egui::Ui, app: &mut App, path: &Path) {
    let thumbnail_path = crate::video::thumbnail_path_for(path);
    let needs_load = app.preview_texture_path.as_deref() != Some(path);
    if needs_load {
        app.preview_texture = None;
        app.preview_error = None;
        match crate::video::extract_thumbnail(path, &thumbnail_path)
            .and_then(|()| load_texture(ui, &thumbnail_path))
        {
            Ok(texture) => app.preview_texture = Some(texture),
            Err(error) => app.preview_error = Some(error.to_string()),
        }
        app.preview_texture_path = Some(path.to_path_buf());
    }

    if let Some(texture) = &app.preview_texture {
        let available = ui.available_size();
        let texture_size = texture.size_vec2();
        let scale = (available.x / texture_size.x)
            .min(available.y / texture_size.y)
            .min(1.0)
            .max(0.05);
        ui.image((texture.id(), texture_size * scale));
    } else {
        ui.label("Video detected, but ffmpeg could not extract a thumbnail.");
        if let Some(error) = &app.preview_error {
            ui.monospace(error);
        }
        ui.monospace(path.display().to_string());
    }
}

fn draw_details(ui: &mut egui::Ui, app: &App) {
    let Some(entry) = app.selected_entry() else {
        ui.label("No file selected.");
        return;
    };

    egui::Grid::new("details_grid")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Name");
            ui.monospace(&entry.file_name);
            ui.end_row();

            ui.label("Path");
            ui.monospace(entry.path.display().to_string());
            ui.end_row();

            ui.label("Kind");
            ui.label(entry.kind.label());
            ui.end_row();

            ui.label("Size");
            ui.label(format_bytes(entry.len));
            ui.end_row();
        });
}

fn kind_icon(kind: CacheKind) -> &'static str {
    match kind {
        CacheKind::Image => "[img]",
        CacheKind::Video => "[vid]",
        CacheKind::Other => "[file]",
    }
}

fn compare_modified(a: Option<SystemTime>, b: Option<SystemTime>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn naturalish_cmp(a: &str, b: &str) -> Ordering {
    a.to_ascii_lowercase()
        .cmp(&b.to_ascii_lowercase())
        .then_with(|| a.cmp(b))
}

fn format_modified(modified: Option<SystemTime>) -> String {
    modified.map_or_else(
        || "unknown".to_string(),
        |modified| {
            let modified: DateTime<Local> = modified.into();
            modified.format("%Y-%m-%d %H:%M:%S").to_string()
        },
    )
}

fn format_bytes(len: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    let len = len as f64;
    if len >= MIB {
        format!("{:.1} MiB", len / MIB)
    } else if len >= KIB {
        format!("{:.1} KiB", len / KIB)
    } else {
        format!("{len:.0} B")
    }
}
