//! 바로보기 창.
//! 속도는 `cargo run --release` 로 본다. 디버그 실행은 일부러 느리다.
//! JPG·PNG 디코더와 GPU만 창을 띄울 때 준비되고, SVG·GIF·변환·썸네일은 그때 호출된다.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use baro::{convert, decode_still, list_images, load_thumb, sniff, visit_gif, Kind, OutFormat, Still};

enum Msg {
    Opened {
        path: PathBuf,
        image: Still,
        ms: f64,
        gif: bool,
        delay_ms: u32,
    },
    GifFrame(Still, u32),
    Thumb(usize, Still),
    Failed(String),
}

struct View {
    path: PathBuf,
    image: Still,
    ms: f64,
    gif: bool,
    frames: Vec<(Still, u32)>,
    frame: usize,
    playing: bool,
}

struct App {
    files: Vec<PathBuf>,
    index: usize,
    view: Option<View>,
    texture: Option<egui::TextureHandle>,
    thumbs: Vec<Option<egui::TextureHandle>>,
    error: Option<String>,
    explorer: bool,
    convert_open: bool,
    quality: u8,
    max_long: Option<u32>,
    format: OutFormat,
    zoom: f32,
    fit: bool,
    pan: egui::Vec2,
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    inflight: usize,
    thumb_queue: std::collections::VecDeque<usize>,
    thumb_started: Vec<bool>,
    play_acc: f32,
    logo: Option<egui::TextureHandle>,
    mark: Option<egui::TextureHandle>,
    percent: i32,
}

impl App {
    fn new(tx: Sender<Msg>, rx: Receiver<Msg>) -> Self {
        Self {
            files: Vec::new(),
            index: 0,
            view: None,
            texture: None,
            thumbs: Vec::new(),
            error: None,
            explorer: false,
            convert_open: false,
            quality: 90,
            max_long: None,
            format: OutFormat::Png,
            zoom: 1.0,
            fit: true,
            pan: egui::Vec2::ZERO,
            rx,
            tx,
            inflight: 0,
            thumb_queue: std::collections::VecDeque::new(),
            thumb_started: Vec::new(),
            play_acc: 0.0,
            logo: None,
            mark: None,
            percent: 100,
        }
    }

    fn ensure_brand(&mut self, ctx: &egui::Context) {
        if self.logo.is_none() {
            if let Some(image) = decode_png(include_bytes!("../assets/nuni-logo.png")) {
                self.logo = Some(ctx.load_texture("nuni-logo", image, egui::TextureOptions::LINEAR));
            }
        }
        if self.mark.is_none() {
            if let Some(image) = raster_svg(include_str!("../assets/nuni-icon.svg"), 128) {
                self.mark = Some(ctx.load_texture("nuni-mark", image, egui::TextureOptions::LINEAR));
            }
        }
    }

    fn load_samples(&mut self) {
        let dir = std::env::temp_dir().join("nuni-view");
        let _ = std::fs::create_dir_all(&dir);
        let png = dir.join("sample.png");
        let svg = dir.join("sample.svg");
        let _ = write_sample_png(&png);
        let _ = std::fs::write(
            &svg,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="160"><rect width="240" height="160" fill="#071018"/><circle cx="120" cy="80" r="46" fill="#0273F5"/></svg>"##,
        );
        self.set_files(vec![png, svg]);
    }

    fn set_files(&mut self, files: Vec<PathBuf>) {
        self.files = files;
        self.index = 0;
        self.thumbs.clear();
        self.thumb_started.clear();
        self.thumb_queue.clear();
        self.thumbs.resize(self.files.len(), None);
        self.thumb_started.resize(self.files.len(), false);
        self.explorer = self.files.len() > 1;
        if let Some(path) = self.files.first().cloned() {
            self.open(path);
        }
    }

    fn open(&mut self, path: PathBuf) {
        self.error = None;
        self.fit = true;
        self.pan = egui::Vec2::ZERO;
        self.play_acc = 0.0;
        let tx = self.tx.clone();
        std::thread::spawn(move || open_file(path, tx));
    }

    fn upload(&mut self, ctx: &egui::Context, image: &Still) {
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width as usize, image.height as usize],
            &image.rgba,
        );
        if let Some(tex) = &mut self.texture {
            tex.set(color, egui::TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture("view", color, egui::TextureOptions::LINEAR));
        }
    }

    fn pump(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Opened {
                    path,
                    image,
                    ms,
                    gif,
                    delay_ms,
                } => {
                    self.upload(ctx, &image);
                    let frames = if gif { vec![(image.clone(), delay_ms)] } else { Vec::new() };
                    self.view = Some(View {
                        path,
                        image,
                        ms,
                        gif,
                        frames,
                        frame: 0,
                        playing: false,
                    });
                }
                Msg::GifFrame(image, delay) => {
                    if let Some(view) = &mut self.view {
                        view.frames.push((image, delay));
                    }
                }
                Msg::Thumb(idx, image) => {
                    self.inflight = self.inflight.saturating_sub(1);
                    if idx < self.thumbs.len() {
                        let color = egui::ColorImage::from_rgba_unmultiplied(
                            [image.width as usize, image.height as usize],
                            &image.rgba,
                        );
                        self.thumbs[idx] =
                            Some(ctx.load_texture(format!("t{idx}"), color, egui::TextureOptions::LINEAR));
                    }
                }
                Msg::Failed(err) => {
                    self.inflight = self.inflight.saturating_sub(1);
                    self.error = Some(err);
                }
            }
        }
        self.kick_thumbs();
        let dt = ctx.input(|i| i.stable_dt);
        let mut acc = self.play_acc;
        let mut advance = None;
        let mut waiting = false;
        if let Some(view) = &mut self.view {
            if view.playing && view.frames.len() > 1 {
                acc += dt * 1000.0;
                let delay = view.frames[view.frame].1 as f32;
                if acc >= delay {
                    acc = 0.0;
                    view.frame = (view.frame + 1) % view.frames.len();
                    advance = Some(view.frames[view.frame].0.clone());
                } else {
                    waiting = true;
                }
            }
        }
        self.play_acc = acc;
        if let Some(image) = advance {
            self.upload(ctx, &image);
            ctx.request_repaint();
        } else if waiting {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn kick_thumbs(&mut self) {
        while self.inflight < 2 {
            let Some(idx) = self.thumb_queue.pop_front() else {
                break;
            };
            if idx >= self.files.len() {
                continue;
            }
            self.inflight += 1;
            let path = self.files[idx].clone();
            let tx = self.tx.clone();
            std::thread::spawn(move || match load_thumb(&path) {
                Ok(image) => {
                    let _ = tx.send(Msg::Thumb(idx, image));
                }
                Err(err) => {
                    let _ = tx.send(Msg::Failed(err));
                }
            });
        }
    }

    fn want_thumb(&mut self, idx: usize) {
        if idx >= self.thumb_started.len() || self.thumb_started[idx] {
            return;
        }
        self.thumb_started[idx] = true;
        self.thumb_queue.push_back(idx);
    }

    fn save(&mut self) {
        let Some(view) = &self.view else { return };
        let stem = view.path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
        let name = format!("{stem}.{}", self.format.extension());
        let source = if view.gif {
            view.frames.first().map(|(image, _)| image).unwrap_or(&view.image).clone()
        } else {
            view.image.clone()
        };
        let format = self.format;
        let quality = self.quality;
        let max_long = self.max_long;
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&name)
            .add_filter("이미지", &[format.extension()])
            .save_file()
        else {
            return;
        };
        match convert(&source, format, quality, max_long) {
            Ok(bytes) => {
                if let Err(err) = std::fs::write(path, bytes) {
                    self.error = Some(err.to_string());
                }
            }
            Err(err) => self.error = Some(err),
        }
    }

    fn pick_files(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("이미지", &["jpg", "jpeg", "png", "gif", "svg"])
            .pick_files()
        {
            self.set_files(paths);
        }
    }

    fn pick_folder(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            self.set_files(list_images(&dir));
        }
    }
}

fn open_file(path: PathBuf, tx: Sender<Msg>) {
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = tx.send(Msg::Failed(err.to_string()));
            return;
        }
    };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let kind = sniff(name, &bytes[..bytes.len().min(256)]);
    let started = Instant::now();
    if kind == Kind::Gif {
        let mut first = true;
        let result = visit_gif(&bytes, |frame| {
            if first {
                first = false;
                let _ = tx.send(Msg::Opened {
                    path: path.clone(),
                    image: frame.image,
                    ms: started.elapsed().as_secs_f64() * 1000.0,
                    gif: true,
                    delay_ms: frame.delay_ms,
                });
            } else {
                let _ = tx.send(Msg::GifFrame(frame.image, frame.delay_ms));
            }
        });
        if let Err(err) = result {
            if first {
                let _ = tx.send(Msg::Failed(err));
            }
        }
        return;
    }
    match decode_still(name, &bytes) {
        Ok(image) => {
            let _ = tx.send(Msg::Opened {
                path,
                image,
                ms: started.elapsed().as_secs_f64() * 1000.0,
                gif: false,
                delay_ms: 0,
            });
        }
        Err(err) => {
            let _ = tx.send(Msg::Failed(err));
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);
        self.ensure_brand(ctx);
        self.pump(ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && self.index > 0 {
            self.index -= 1;
            if let Some(path) = self.files.get(self.index).cloned() {
                self.open(path);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) && self.index + 1 < self.files.len() {
            self.index += 1;
            if let Some(path) = self.files.get(self.index).cloned() {
                self.open(path);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.fit = true;
            self.pan = egui::Vec2::ZERO;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
            self.fit = false;
            self.zoom = 1.0;
            self.pan = egui::Vec2::ZERO;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::C)) {
            self.convert_open = !self.convert_open;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O)) {
            self.pick_files();
        }

        let bar = egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::symmetric(8, 8));
        let top = egui::TopBottomPanel::top("top")
            .exact_height(56.0)
            .frame(bar)
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.horizontal_centered(|ui| {
                    if let Some(tex) = &self.logo {
                        let [w, h] = tex.size();
                        let height = 40.0;
                        let width = height * w as f32 / h.max(1) as f32;
                        ui.image((tex.id(), egui::vec2(width, height)));
                    }
                    ui.label(egui::RichText::new("view").color(ACCENT).size(14.0));
                    if bar_button(ui, "열기", false).clicked() {
                        self.pick_files();
                    }
                    if bar_button(ui, "폴더", false).clicked() {
                        self.pick_folder();
                    }
                    if bar_button_color(ui, "샘플", MUTED, BG).clicked() {
                        self.load_samples();
                    }
                    let name = self
                        .view
                        .as_ref()
                        .map(|view| file_label(&view.path))
                        .unwrap_or_else(|| "파일 없음".to_string());
                    let meta = self.view.as_ref().map(|view| meta_line(view, self.percent));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(meta) = meta {
                            ui.label(egui::RichText::new(meta).color(MUTED).size(12.0));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(name).size(14.0)).truncate());
                        });
                    });
                });
            });
        hairline(ctx, top.response.rect.left_bottom(), top.response.rect.right_bottom());

        let bottom = egui::TopBottomPanel::bottom("bottom")
            .exact_height(56.0)
            .frame(bar)
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.horizontal_centered(|ui| {
                    if bar_button_enabled(ui, "<", false, self.index > 0).clicked() && self.index > 0 {
                        self.index -= 1;
                        if let Some(path) = self.files.get(self.index).cloned() {
                            self.open(path);
                        }
                    }
                    if bar_button_enabled(ui, ">", false, self.index + 1 < self.files.len()).clicked()
                        && self.index + 1 < self.files.len()
                    {
                        self.index += 1;
                        if let Some(path) = self.files.get(self.index).cloned() {
                            self.open(path);
                        }
                    }
                    if bar_button(ui, "맞춤", false).clicked() {
                        self.fit = true;
                        self.pan = egui::Vec2::ZERO;
                    }
                    if bar_button(ui, "1:1", false).clicked() {
                        self.fit = false;
                        self.zoom = 1.0;
                        self.pan = egui::Vec2::ZERO;
                    }
                    if bar_button_enabled(ui, "목록", false, !self.files.is_empty()).clicked() {
                        self.explorer = !self.explorer;
                    }
                    let gif_toggle = self.view.as_ref().and_then(|view| {
                        view.gif
                            .then_some((!view.frames.is_empty(), view.playing, view.frame, view.frames.len().max(1)))
                    });
                    if let Some((ready, playing, frame, total)) = gif_toggle {
                        let label = if playing { "일시정지" } else { "재생" };
                        if bar_button_enabled(ui, label, false, ready).clicked() {
                            if let Some(view) = &mut self.view {
                                view.playing = !view.playing;
                            }
                        }
                        ui.label(
                            egui::RichText::new(format!("{}/{}", frame + 1, total))
                                .color(MUTED)
                                .size(12.0),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if bar_button_enabled(ui, "변환", self.convert_open, self.view.is_some()).clicked() {
                            self.convert_open = !self.convert_open;
                        }
                    });
                });
            });
        hairline(ctx, bottom.response.rect.left_top(), bottom.response.rect.right_top());

        if self.explorer && !self.files.is_empty() {
            let len = self.files.len();
            let names: Vec<String> = self.files.iter().map(|path| file_label(path)).collect();
            let mut pick = None;
            let mut wanted = Vec::new();
            egui::SidePanel::left("files")
                .exact_width(256.0)
                .frame(
                    egui::Frame::new()
                        .fill(SURFACE)
                        .inner_margin(egui::Margin::symmetric(8, 8)),
                )
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(format!("{len}개")).color(MUTED).size(12.0));
                    egui::ScrollArea::vertical().show_rows(ui, 40.0, len, |ui, range| {
                        for idx in range {
                            wanted.push(idx);
                            ui.horizontal(|ui| {
                                let size = egui::vec2(28.0, 28.0);
                                if let Some(Some(tex)) = self.thumbs.get(idx) {
                                    ui.image((tex.id(), size));
                                } else {
                                    ui.allocate_space(size);
                                }
                                if ui.selectable_label(idx == self.index, &names[idx]).clicked() {
                                    pick = Some(idx);
                                }
                            });
                        }
                    });
                });
            for idx in wanted {
                self.want_thumb(idx);
            }
            if let Some(idx) = pick {
                self.index = idx;
                if let Some(path) = self.files.get(idx).cloned() {
                    self.open(path);
                }
            }
        }

        let (iw, ih, missing) = match &self.view {
            Some(view) => (view.image.width as f32, view.image.height as f32, false),
            None => (0.0, 0.0, true),
        };
        let error = self.error.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                paint_checker(ui.painter(), rect);
                if missing {
                    paint_empty(ui.painter(), rect, self.mark.as_ref());
                } else if let Some(tex_id) = self.texture.as_ref().map(|tex| tex.id()) {
                    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                    if response.dragged() {
                        self.pan += response.drag_delta();
                        self.fit = false;
                    }
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if response.hovered() && scroll.abs() > 0.0 {
                        self.fit = false;
                        let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
                        self.zoom = (self.zoom * factor).clamp(0.05, 32.0);
                    }
                    let scale = if self.fit {
                        (rect.width() / iw).min(rect.height() / ih)
                    } else {
                        self.zoom
                    };
                    self.percent = (scale * 100.0).round() as i32;
                    let dest =
                        egui::Rect::from_center_size(rect.center() + self.pan, egui::vec2(iw * scale, ih * scale));
                    ui.painter().image(
                        tex_id,
                        dest,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                if let Some(err) = error {
                    ui.painter().text(
                        rect.left_bottom() + egui::vec2(12.0, -28.0),
                        egui::Align2::LEFT_CENTER,
                        err,
                        egui::FontId::proportional(14.0),
                        DANGER,
                    );
                }
            });

        let mut do_save = false;
        if self.convert_open {
            let gif = self.view.as_ref().is_some_and(|view| view.gif);
            let svg = self.view.as_ref().is_some_and(|view| {
                view.path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
            });
            egui::Window::new("변환")
                .collapsible(false)
                .title_bar(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -68.0))
                .fixed_size(egui::vec2(300.0, 0.0))
                .frame(
                    egui::Frame::new()
                        .fill(SURFACE)
                        .stroke(egui::Stroke::new(1.0, BORDER))
                        .inner_margin(egui::Margin::same(12))
                        .corner_radius(8),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("변환").size(14.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if bar_button_color(ui, "닫기", MUTED, SURFACE).clicked() {
                                self.convert_open = false;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        for (fmt, label) in
                            [(OutFormat::Png, "PNG"), (OutFormat::Jpeg, "JPEG"), (OutFormat::Webp, "WebP")]
                        {
                            if bar_button(ui, label, self.format == fmt).clicked() {
                                self.format = fmt;
                            }
                        }
                    });
                    ui.add_space(8.0);
                    let quality_on = self.format != OutFormat::Png;
                    ui.label(
                        egui::RichText::new(if quality_on {
                            format!("품질 {}", self.quality)
                        } else {
                            "품질 —".to_string()
                        })
                        .color(MUTED)
                        .size(14.0),
                    );
                    ui.add_enabled(quality_on, egui::Slider::new(&mut self.quality, 40..=100));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("긴 변").color(MUTED).size(14.0));
                    ui.horizontal(|ui| {
                        for (label, value) in
                            [("원본", None), ("1920", Some(1920u32)), ("1280", Some(1280)), ("800", Some(800))]
                        {
                            if bar_button(ui, label, self.max_long == value).clicked() {
                                self.max_long = value;
                            }
                        }
                    });
                    if gif {
                        ui.label(egui::RichText::new("GIF는 첫 프레임만 저장합니다.").color(MUTED).size(13.0));
                    }
                    if svg {
                        ui.label(egui::RichText::new("SVG는 픽셀로 굽습니다.").color(MUTED).size(13.0));
                    }
                    ui.add_space(8.0);
                    if bar_button(ui, "저장", true).clicked() {
                        do_save = true;
                    }
                });
        }
        if do_save {
            self.save();
            self.convert_open = false;
        }
    }
}

fn file_label(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

fn meta_line(view: &View, percent: i32) -> String {
    let bytes = std::fs::metadata(&view.path).map(|meta| meta.len()).unwrap_or(0);
    format!(
        "{}×{}  ·  {percent}%  ·  {}  ·  {:.0}ms",
        view.image.width,
        view.image.height,
        format_bytes(bytes),
        view.ms
    )
}

fn format_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1}MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{}KB", (n as f64 / 1024.0).round())
    } else {
        format!("{n}B")
    }
}

fn paint_checker(painter: &egui::Painter, rect: egui::Rect) {
    let cell = 16.0;
    let mut y = rect.top();
    let mut row = 0i32;
    while y < rect.bottom() {
        let mut x = rect.left();
        let mut col = 0i32;
        while x < rect.right() {
            let color = if (row + col) % 2 == 0 { CHECK_A } else { CHECK_B };
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x, y),
                    egui::pos2((x + cell).min(rect.right()), (y + cell).min(rect.bottom())),
                ),
                0.0,
                color,
            );
            x += cell;
            col += 1;
        }
        y += cell;
        row += 1;
    }
}

fn paint_empty(painter: &egui::Painter, rect: egui::Rect, mark: Option<&egui::TextureHandle>) {
    let center = rect.center();
    if let Some(mark) = mark {
        painter.image(
            mark.id(),
            egui::Rect::from_center_size(center - egui::vec2(0.0, 42.0), egui::vec2(64.0, 64.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    painter.text(
        center + egui::vec2(0.0, 16.0),
        egui::Align2::CENTER_CENTER,
        "이미지를 놓거나 열기를 누르세요",
        egui::FontId::proportional(16.0),
        FG,
    );
    painter.text(
        center + egui::vec2(0.0, 42.0),
        egui::Align2::CENTER_CENTER,
        "← → 이동 · F 맞춤 · 1 원본 · C 변환 · O 열기",
        egui::FontId::proportional(13.0),
        MUTED,
    );
}

fn hairline(ctx: &egui::Context, a: egui::Pos2, b: egui::Pos2) {
    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("nuni-chrome")))
        .line_segment([a, b], egui::Stroke::new(1.0, BORDER));
}

fn bar_button(ui: &mut egui::Ui, text: &str, accent: bool) -> egui::Response {
    let fill = if accent { ACCENT } else { SURFACE2 };
    let color = if accent { WHITE } else { FG };
    bar_button_color(ui, text, color, fill)
}

fn bar_button_color(ui: &mut egui::Ui, text: &str, color: egui::Color32, fill: egui::Color32) -> egui::Response {
    ui.add(chrome_button(text, color, fill))
}

fn bar_button_enabled(ui: &mut egui::Ui, text: &str, accent: bool, enabled: bool) -> egui::Response {
    let fill = if accent { ACCENT } else { SURFACE2 };
    let color = if accent { WHITE } else { FG };
    ui.add_enabled(enabled, chrome_button(text, color, fill))
}

fn chrome_button(text: &str, color: egui::Color32, fill: egui::Color32) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(text).size(14.0).color(color))
        .fill(fill)
        .stroke(egui::Stroke::NONE)
        .corner_radius(8)
        .min_size(egui::vec2(44.0, 36.0))
}

fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let visuals = nuni_visuals();
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals_of(egui::Theme::Light, visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing.x = 8.0;
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
    });
}

fn nuni_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    let radius = egui::CornerRadius::same(8);
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = SURFACE;
    visuals.code_bg_color = SURFACE2;
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = egui::Stroke::NONE;
    visuals.hyperlink_color = egui::Color32::from_rgb(122, 210, 255);
    visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
    visuals.window_corner_radius = radius;
    visuals.menu_corner_radius = radius;
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, FG);
    visuals.widgets.inactive.bg_fill = SURFACE2;
    visuals.widgets.inactive.weak_bg_fill = SURFACE2;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, FG);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(18, 48, 74);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(18, 48, 74);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, FG);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.weak_bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, WHITE);
    visuals.widgets.open.bg_fill = SURFACE2;
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, FG);
    visuals
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "plex".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/IBMPlexSansKR-Regular.ttf"
        ))),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "plex".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.insert(0, "plex".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn decode_png(bytes: &[u8]) -> Option<egui::ColorImage> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = image.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], image.as_raw()))
}

fn raster_svg(svg: &str, px: u32) -> Option<egui::ColorImage> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let longest = size.width().max(size.height()).max(1.0);
    let scale = px as f32 / longest;
    let w = (size.width() * scale).ceil().max(1.0) as u32;
    let h = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Some(egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], pixmap.data()))
}

fn app_icon() -> Option<std::sync::Arc<egui::IconData>> {
    let image = raster_svg(include_str!("../assets/nuni-icon.svg"), 128)?;
    Some(std::sync::Arc::new(egui::IconData {
        rgba: image.pixels.iter().flat_map(|pixel| pixel.to_array()).collect(),
        width: image.size[0] as u32,
        height: image.size[1] as u32,
    }))
}

fn write_sample_png(path: &Path) -> bool {
    let (w, h) = (320u32, 200u32);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let dx = x as f32 - 110.0;
            let dy = y as f32 - 100.0;
            let on = dx * dx + dy * dy < 46.0 * 46.0;
            data[i] = if on { 2 } else { 7 };
            data[i + 1] = if on { 115 } else { 16 };
            data[i + 2] = if on { 245 } else { 24 };
            data[i + 3] = 255;
        }
    }
    let Ok(file) = std::fs::File::create(path) else {
        return false;
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let Ok(mut writer) = encoder.write_header() else {
        return false;
    };
    writer.write_image_data(&data).is_ok()
}

const BG: egui::Color32 = egui::Color32::from_rgb(0, 0, 0);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(7, 16, 24);
const SURFACE2: egui::Color32 = egui::Color32::from_rgb(13, 28, 46);
const FG: egui::Color32 = egui::Color32::from_rgb(243, 251, 255);
const MUTED: egui::Color32 = egui::Color32::from_rgb(122, 164, 196);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(2, 115, 245);
const WHITE: egui::Color32 = egui::Color32::WHITE;
const BORDER: egui::Color32 = egui::Color32::from_rgb(18, 48, 74);
const CHECK_A: egui::Color32 = egui::Color32::from_rgb(5, 8, 12);
const CHECK_B: egui::Color32 = egui::Color32::from_rgb(12, 24, 36);
const DANGER: egui::Color32 = egui::Color32::from_rgb(255, 141, 122);

fn main() -> eframe::Result {
    let (tx, rx) = mpsc::channel();
    let mut viewport = egui::ViewportBuilder::default().with_inner_size([1100.0, 720.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "누니뷰",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(App::new(tx, rx)))
        }),
    )
}
