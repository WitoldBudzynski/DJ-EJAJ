use crate::{
    controls::*,
    state::{self, Binding, Session},
};
use djejaj_audio::{Command, Engine, RecordEvent, RecordRequest, SourceEvent};
use djejaj_library::{Event, Library, Query, Request, Track};
use djejaj_sampler::{KEYS, NAMES, Step};
use eframe::egui::{self, Align, Color32, FontId, Frame, Key, Layout, RichText, Stroke, Vec2};
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Context {
    Play,
    Library,
    Sequence,
}
type SampleResult = (
    usize,
    PathBuf,
    bool,
    Result<Vec<djejaj_audio::Frame>, String>,
);
pub struct App {
    engine: Option<Engine>,
    library: Library,
    session: Session,
    tracks: Vec<Track>,
    selected: Option<i64>,
    pending: [Option<(u64, Track)>; 3],
    context: Context,
    active: usize,
    live: bool,
    search: String,
    tag: String,
    unplayed: bool,
    bpm_filter: bool,
    key_filter: bool,
    query: u64,
    status: String,
    scanning: bool,
    seq_open: bool,
    seq_running: bool,
    seq_track: usize,
    seq_step: usize,
    seq_page: usize,
    seq_pattern: [usize; 8],
    show_settings: bool,
    show_help: bool,
    edit_track: Option<Track>,
    device: String,
    buffer: u32,
    cue_enabled: bool,
    bindings: Vec<Binding>,
    stop_since: Option<Instant>,
    pointer_stop: [Option<Instant>; 2],
    keys_blocked: bool,
    preview_track: Option<Track>,
    record_path: Option<PathBuf>,
    played: [f64; 2],
    last_frame: Instant,
    screenshot: Option<PathBuf>,
    frames: u64,
    demo_pending: bool,
    undo: Vec<(usize, usize, usize, Step)>,
    session_valid: bool,
    spectrum: Spectrum,
    playlists: Vec<(i64, String)>,
    playlist: Option<i64>,
    playlist_name: String,
    related: bool,
    sample_jobs: crossbeam_channel::Sender<(usize, PathBuf, u32, bool)>,
    sample_results: crossbeam_channel::Receiver<SampleResult>,
    sample_loading: usize,
    scene_select: usize,
    last_scene: usize,
    taps: std::collections::VecDeque<Instant>,
    dir: PathBuf,
    browser_rows: usize,
    layout_metrics: [f32; 5],
    readiness: Vec<String>,
}
fn panel() -> Frame {
    Frame::new()
        .fill(PANEL)
        .inner_margin(9.0)
        .corner_radius(5.0)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(53, 60, 66)))
}
fn time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        demo: bool,
        screenshot: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        Self::init(&cc.egui_ctx, demo, screenshot, state::data_dir(), true)
    }
    fn init(
        ctx: &egui::Context,
        demo: bool,
        screenshot: Option<PathBuf>,
        dir: PathBuf,
        audio: bool,
    ) -> anyhow::Result<Self> {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(19, 23, 27);
        style.visuals.window_fill = PANEL;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = Color32::from_rgb(36, 66, 82);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(48, 84, 104);
        style.spacing.item_spacing = Vec2::new(6.0, 4.0);
        style.spacing.button_padding = Vec2::new(8.0, 5.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(11.0));
        ctx.set_style(style);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("session.json");
        let mut session_valid = true;
        let mut status =
            "Wskaż katalog muzyczny, aby zbudować bibliotekę. Audio startuje bez odtwarzania."
                .to_owned();
        let session = if path.exists() {
            match state::load(&path) {
                Ok(s) => s,
                Err(e) => {
                    session_valid = false;
                    status = format!("Nie można odczytać sesji: {e}. Plik pozostaje zachowany.");
                    Session::default()
                }
            }
        } else {
            Session::default()
        };
        let engine = if audio {
            match Engine::new(None, 256, false) {
                Ok(e) => Some(e),
                Err(e) => {
                    status = format!("Audio niedostępne: {e}. Sprawdź Ustawienia audio.");
                    None
                }
            }
        } else {
            None
        };
        let library = Library::open(dir.join("library.sqlite3"))?;
        let (sample_jobs, jobs) = crossbeam_channel::bounded::<(usize, PathBuf, u32, bool)>(8);
        let (results, sample_results) = crossbeam_channel::bounded::<SampleResult>(8);
        std::thread::Builder::new()
            .name("dj-sample-import".into())
            .spawn(move || {
                while let Ok((kind, path, sr, imported)) = jobs.recv() {
                    let result = djejaj_audio::prepare_sample(&path, sr).map_err(|e| e.to_string());
                    if results.send((kind, path, imported, result)).is_err() {
                        break;
                    }
                }
            })?;
        let mut app = Self {
            engine,
            library,
            bindings: session.bindings.clone(),
            session,
            tracks: Vec::new(),
            selected: None,
            pending: std::array::from_fn(|_| None),
            context: Context::Play,
            active: 0,
            live: true,
            search: String::new(),
            tag: String::new(),
            unplayed: false,
            bpm_filter: false,
            key_filter: false,
            query: 0,
            status,
            scanning: false,
            seq_open: true,
            seq_running: false,
            seq_track: 0,
            seq_step: 0,
            seq_page: 0,
            seq_pattern: [1; 8],
            show_settings: false,
            show_help: false,
            edit_track: None,
            device: String::new(),
            buffer: 256,
            cue_enabled: false,
            stop_since: None,
            pointer_stop: [None; 2],
            keys_blocked: false,
            preview_track: None,
            record_path: None,
            played: [0.0; 2],
            last_frame: Instant::now(),
            screenshot,
            frames: 0,
            demo_pending: demo,
            undo: Vec::new(),
            session_valid,
            spectrum: Spectrum::new(),
            playlists: Vec::new(),
            playlist: None,
            playlist_name: String::new(),
            related: false,
            sample_jobs,
            sample_results,
            sample_loading: 0,
            scene_select: 1,
            last_scene: 1,
            taps: std::collections::VecDeque::new(),
            dir,
            browser_rows: 0,
            layout_metrics: [0.0; 5],
            readiness: Vec::new(),
        };
        if let Some(e) = &app.engine {
            let _ = e.settings(app.session.settings.clone());
        }
        let _ = app.library.requests.send(Request::Playlists);
        app.refresh();
        for i in 0..2 {
            if let Some(track) = app.session.tracks[i].clone() {
                app.load_track(i, track);
            }
        }
        app.restore_sequence();
        Ok(app)
    }
    fn command(&mut self, c: Command) {
        if let Some(e) = &self.engine
            && let Err(err) = e.shared.send(c)
        {
            self.status = err.to_string();
        }
    }
    fn restore_sequence(&mut self) {
        if let Some(e) = &self.engine {
            for (i, c) in self.session.sample_configs.iter().enumerate() {
                let _ = e.shared.send(Command::SampleConfig(i, *c));
            }
            for (track, patterns) in self.session.sequence.patterns.iter().enumerate() {
                for (pattern, p) in patterns.iter().enumerate() {
                    let _ = e.shared.send(Command::Length {
                        track,
                        pattern,
                        length: p.length,
                    });
                    for (index, value) in p.steps.iter().enumerate() {
                        if *value != Step::default() {
                            let _ = e.shared.send(Command::Step {
                                track,
                                pattern,
                                index,
                                value: *value,
                            });
                        }
                    }
                }
            }
            for (scene, s) in self.session.sequence.scenes.iter().enumerate() {
                for (track, p) in s.patterns.iter().enumerate() {
                    let _ = e.shared.send(Command::ScenePattern {
                        scene,
                        track,
                        pattern: *p,
                    });
                    let _ = e.shared.send(Command::SceneMute {
                        scene,
                        track,
                        muted: s.muted[track],
                    });
                }
            }
        }
        for (i, path) in self.session.sample_paths.clone().into_iter().enumerate() {
            if let Some(path) = path {
                self.queue_sample(i, PathBuf::from(path), false);
            }
        }
    }
    fn refresh(&mut self) {
        self.query += 1;
        let bpm = self.session.settings.bpm;
        let key = if self.key_filter {
            self.session.tracks[self.active]
                .as_ref()
                .and_then(|t| t.key.clone())
        } else {
            None
        };
        let q = Query {
            text: self.search.clone(),
            bpm_min: self.bpm_filter.then_some(bpm - 4.0),
            bpm_max: self.bpm_filter.then_some(bpm + 4.0),
            key,
            unplayed: self.unplayed,
            tag: self.tag.clone(),
            playlist: self.playlist,
        };
        let _ = self.library.requests.send(Request::Query(self.query, q));
    }
    fn selected_track(&self) -> Option<Track> {
        self.selected
            .and_then(|id| self.tracks.iter().find(|t| t.id == id))
            .cloned()
    }
    fn load_track(&mut self, index: usize, track: Track) {
        if track.missing || track.error.is_some() {
            self.status = track
                .error
                .clone()
                .unwrap_or_else(|| "Plik jest niedostępny".into());
            return;
        }
        if index == 2 && !self.engine.as_ref().is_some_and(|e| e.channels >= 4) {
            self.status =
                "Odsłuch wymaga wyjścia 4-kanałowego. Wybierz je w Ustawieniach audio.".into();
            return;
        }
        if let Some(e) = &self.engine {
            match e.sources[index].load(PathBuf::from(&track.path), self.live) {
                Ok(g) => {
                    self.pending[index] = Some((g, track));
                    self.status = format!(
                        "Przygotowywanie {}…",
                        if index == 2 {
                            "odsłuchu"
                        } else if index == 0 {
                            "decka A"
                        } else {
                            "decka B"
                        }
                    );
                }
                Err(e) => self.status = e.to_string(),
            }
        } else {
            self.status = "Najpierw uruchom wyjście audio w Ustawieniach.".into();
        }
    }
    fn start(&mut self, index: usize) {
        if let Some(e) = &self.engine {
            if e.sources[index].control.playing.load(Ordering::Relaxed) {
                if !self.live {
                    e.sources[index].stop();
                } else {
                    self.status =
                        "LIVE: zatrzymanie wymaga przytrzymania Shift+Spacja lub przycisku STOP."
                            .into();
                }
            } else {
                if let Some(t) = self.session.tracks.get(index).and_then(|t| t.as_ref())
                    && t.duration > 0.0
                    && e.sources[index].position() >= t.duration - 0.1
                {
                    e.sources[index].seek(0.0);
                }
                if let Err(err) = e.sources[index].play() {
                    self.status = err.to_string();
                }
            }
        }
    }
    fn panic(&mut self) {
        if let Some(e) = &self.engine {
            e.shared.panic();
        }
        self.seq_running = false;
        self.stop_since = None;
        self.status =
            "PANIC · wszystkie źródła wyciszone. Uruchom wybrane źródło, aby wrócić.".into();
    }
    fn queue_sample(&mut self, kind: usize, path: PathBuf, imported: bool) {
        let sr = self.engine.as_ref().map_or(48000, |e| e.sample_rate);
        match self.sample_jobs.try_send((kind, path, sr, imported)) {
            Ok(()) => {
                self.sample_loading += 1;
                self.status = "Przygotowanie sampla w tle…".into();
            }
            Err(_) => self.status = "Kolejka importu sampli jest pełna".into(),
        }
    }
    fn poll(&mut self) {
        if let Some(e) = &self.engine {
            while let Some(data) = e.shared.retired.pop() {
                drop(data);
            }
        }
        while let Ok((kind, path, imported, result)) = self.sample_results.try_recv() {
            self.sample_loading = self.sample_loading.saturating_sub(1);
            match result {
                Ok(data) => {
                    self.command(Command::ReplaceSample(kind, data));
                    if kind == 5 && imported {
                        self.session.sample_configs[5].synth = false;
                        self.command(Command::SampleConfig(5, self.session.sample_configs[5]));
                    }
                    self.session.sample_paths[kind] = Some(path.to_string_lossy().into_owned());
                    self.status = format!("Sample gotowy: {}", path.display());
                }
                Err(error) => self.status = error,
            }
        }

        while let Ok(event) = self.library.events.try_recv() {
            match event {
                Event::Rows(id, mut rows) if id == self.query => {
                    if self.related
                        && let Some(reference) = &self.session.tracks[self.active]
                    {
                        let d = self.session.settings.decks[self.active];
                        rows.sort_by(|a, b| {
                            djejaj_library::music::suggestion(
                                reference,
                                b,
                                self.session.settings.bpm,
                                d.speed,
                                d.key_lock,
                            )
                            .0
                            .total_cmp(
                                &djejaj_library::music::suggestion(
                                    reference,
                                    a,
                                    self.session.settings.bpm,
                                    d.speed,
                                    d.key_lock,
                                )
                                .0,
                            )
                        });
                    }
                    self.tracks = rows;
                    if self.selected.is_none() {
                        self.selected = self.tracks.first().map(|t| t.id);
                    }
                    if self.demo_pending && !self.tracks.is_empty() {
                        self.demo_pending = false;
                        for i in 0..2.min(self.tracks.len()) {
                            self.load_track(i, self.tracks[i].clone());
                        }
                    }
                }
                Event::Rows(..) => {}
                Event::Progress(n) => self.status = format!("Indeksowanie: {n} utworów…"),
                Event::Scanned(n) => {
                    self.scanning = false;
                    self.status = format!(
                        "Biblioteka zaktualizowana: {n} plików. Wybierz utwór i załaduj A/B."
                    );
                    self.refresh();
                }
                Event::Playlists(rows) => {
                    self.playlists = rows;
                    if self
                        .playlist
                        .is_some_and(|id| !self.playlists.iter().any(|(p, _)| *p == id))
                    {
                        self.playlist = None;
                        self.refresh();
                    }
                }
                Event::Detailed(track) => {
                    let track = *track;
                    let loaded = self
                        .session
                        .tracks
                        .iter()
                        .flatten()
                        .any(|t| t.path == track.path);
                    for t in self.session.tracks.iter_mut().flatten() {
                        if t.path == track.path {
                            *t = track.clone();
                        }
                    }
                    if let Some(t) = &mut self.edit_track
                        && t.path == track.path
                    {
                        t.analysis = track.analysis.clone();
                        t.markers = track.markers.clone();
                        t.phrase_start = track.phrase_start;
                        t.phrase_bars = track.phrase_bars;
                    }
                    if loaded && track.analysis.is_none() {
                        let _ = self.library.requests.send(Request::Analyze(track.id));
                    }
                }
                Event::Analyzed(track) => {
                    let track = *track;
                    for loaded in self.session.tracks.iter_mut().flatten() {
                        if loaded.id == track.id {
                            *loaded = track.clone();
                        }
                    }
                    self.status = format!(
                        "Analiza gotowa: {} · BPM {} · tonacja {}",
                        track.title,
                        track
                            .bpm
                            .map_or("nieznane".to_owned(), |b| format!("{b:.1}")),
                        track.key.as_deref().unwrap_or("nieznana")
                    );
                    self.refresh();
                }
                Event::Error(e) => {
                    self.scanning = false;
                    self.status = e;
                }
                Event::Saved => self.refresh(),
            }
        }
        let mut source_events = Vec::new();
        if let Some(e) = &self.engine {
            for i in 0..3 {
                while let Ok(event) = e.sources[i].events.try_recv() {
                    source_events.push((i, event));
                }
            }
        }
        for (i, event) in source_events {
            match event {
                SourceEvent::Ready {
                    generation,
                    path,
                    duration,
                    sample_rate,
                } => {
                    if self.pending[i]
                        .as_ref()
                        .is_some_and(|(g, _)| *g == generation)
                    {
                        let (_, track) = self.pending[i].take().unwrap();
                        self.status = format!("Gotowy: {}", track.title);
                        if i < 2 {
                            self.played[i] = 0.0;
                            self.session.tracks[i] = Some(track.clone());
                            let _ = self
                                .library
                                .requests
                                .send(Request::Details(track.path.clone()));
                        } else {
                            self.preview_track = Some(track);
                            if let Some(e) = &self.engine {
                                let _ = e.sources[2].play();
                            }
                        }
                    }
                    if i < 2
                        && self.pending[i].is_none()
                        && self.engine.as_ref().is_some_and(|e| {
                            e.sources[i].control.generation.load(Ordering::Acquire) == generation
                        })
                        && self.session.tracks[i]
                            .as_ref()
                            .is_none_or(|t| std::path::Path::new(&t.path) != path)
                    {
                        let title = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Utwór")
                            .to_owned();
                        let track = self
                            .tracks
                            .iter()
                            .find(|t| std::path::Path::new(&t.path) == path)
                            .cloned()
                            .unwrap_or_else(|| Track {
                                path: path.to_string_lossy().into_owned(),
                                title,
                                duration,
                                sample_rate,
                                phrase_bars: 16,
                                ..Track::default()
                            });
                        self.status = format!(
                            "Zachowano ostatni poprawnie załadowany utwór: {}",
                            track.title
                        );
                        let _ = self
                            .library
                            .requests
                            .send(Request::Details(track.path.clone()));
                        self.session.tracks[i] = Some(track);
                    }
                }
                SourceEvent::Error {
                    generation,
                    message,
                } => {
                    if self.pending[i]
                        .as_ref()
                        .is_some_and(|(g, _)| *g == generation)
                    {
                        self.pending[i] = None;
                    }
                    self.status = format!("Odczyt audio: {message}");
                }
            }
        }
        if let Some(e) = &self.engine {
            while let Ok(event) = e.recorder.events.try_recv() {
                match event {
                    RecordEvent::Started(p) => {
                        self.status = format!("Nagrywanie: {}", p.display());
                        self.record_path = Some(p);
                    }
                    RecordEvent::Finished(p, frames) => {
                        self.status = format!(
                            "Zapisano WAV stereo: {} ({})",
                            p.display(),
                            time(frames as f64 / e.sample_rate as f64)
                        )
                    }
                    RecordEvent::Error(err) => self.status = format!("Błąd nagrania: {err}"),
                }
            }
            if e.shared.error.load(Ordering::Relaxed) {
                self.status =
                    "Połączenie audio przerwane. Wyjście jest wyciszone; sprawdź Ustawienia audio."
                        .into();
            }
        }
    }
    fn save_session(&mut self) {
        if let Err(e) = state::validate_bindings(&self.bindings) {
            self.status = e.to_string();
            return;
        }
        self.session.bindings = self.bindings.clone();
        match state::save(&self.dir.join("session.json"), &self.session) {
            Ok(()) => {
                self.session_valid = true;
                self.status =
                    "Sesja zapisana. Przy otwarciu transporty pozostaną zatrzymane.".into();
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn keys(&mut self, ctx: &egui::Context, dt: f32) {
        let input = ctx.input(|i| i.clone());
        let command = input.modifiers.command;
        for (k, kind) in [
            (Key::Z, 0),
            (Key::X, 1),
            (Key::C, 2),
            (Key::V, 3),
            (Key::B, 4),
            (Key::N, 5),
            (Key::M, 6),
            (Key::Comma, 7),
        ] {
            if input.key_released(k) {
                self.command(Command::PadOff(kind));
            }
        }
        if !command
            && self.context == Context::Play
            && !ctx.wants_keyboard_input()
            && input.key_pressed(Key::P)
        {
            self.tap_tempo();
        }
        if command && input.key_pressed(Key::Period) {
            self.panic();
            return;
        }
        if command && input.key_pressed(Key::S) {
            self.save_session();
            return;
        }
        if command && input.key_pressed(Key::L) {
            self.context = Context::Library;
        }
        if command && input.key_pressed(Key::J) {
            self.context = Context::Play;
            ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(egui::Id::NULL)));
        }
        if command && input.key_pressed(Key::K) {
            self.context = Context::Sequence;
            self.seq_open = true;
        }
        if command && input.key_pressed(Key::F) {
            self.context = Context::Library;
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("library-search")));
        }
        if command
            && input.key_pressed(Key::Num1)
            && let Some(t) = self.selected_track()
        {
            self.load_track(0, t);
        }
        if command
            && input.key_pressed(Key::Num2)
            && let Some(t) = self.selected_track()
        {
            self.load_track(1, t);
        }
        if input.key_pressed(Key::Escape) {
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            self.context = Context::Play;
            self.edit_track = None;
        }
        if !input.focused {
            self.stop_since = None;
            self.keys_blocked = true;
            self.command(Command::ReleasePads);
            return;
        }
        if self.keys_blocked {
            if input.keys_down.is_empty() {
                self.keys_blocked = false;
            }
            return;
        }
        if command
            || self.show_settings
            || self.show_help
            || self.edit_track.is_some()
            || ctx.wants_keyboard_input()
        {
            self.stop_since = None;
            return;
        }
        match self.context {
            Context::Play => {
                for (k, i) in [(Key::Num1, 0), (Key::Num2, 1)] {
                    if input.key_pressed(k) {
                        self.active = i;
                        self.keys_blocked = true;
                        return;
                    }
                }
                if input.key_down(Key::Space) && input.modifiers.shift {
                    let since = self.stop_since.get_or_insert_with(Instant::now);
                    if since.elapsed().as_millis() >= 700 {
                        if let Some(e) = &self.engine {
                            e.sources[self.active].stop();
                        }
                        self.status = "Deck zatrzymany.".into();
                    }
                } else {
                    self.stop_since = None;
                    if input.key_pressed(Key::Space) {
                        self.start(self.active);
                    }
                }
                let fine = if input.modifiers.shift { 0.15 } else { 1.0 };
                let bindings = if state::validate_bindings(&self.bindings).is_ok() {
                    &self.bindings
                } else {
                    &self.session.bindings
                };
                for (index, b) in bindings.iter().enumerate() {
                    let less = Key::from_name(&b.less.to_uppercase());
                    let more = Key::from_name(&b.more.to_uppercase());
                    let delta = (f32::from(more.is_some_and(|k| input.key_down(k)))
                        - f32::from(less.is_some_and(|k| input.key_down(k))))
                        * dt
                        * fine;
                    let d = &mut self.session.settings.decks[self.active];
                    match index {
                        0 => {
                            if delta != 0.0 {
                                d.filter_on = true;
                                d.cutoff = (d.cutoff * (delta * 3.5).exp()).clamp(25.0, 20000.0);
                            }
                        }
                        1 => d.delay = (d.delay + delta * 0.4).clamp(0.0, 1.0),
                        2 => d.reverb = (d.reverb + delta * 0.4).clamp(0.0, 1.0),
                        3 => d.drive = (d.drive + delta * 0.4).clamp(0.0, 1.0),
                        4 => {
                            d.speed = (d.speed + delta * 0.08).clamp(
                                if d.key_lock { 0.8 } else { 0.5 },
                                if d.key_lock { 1.2 } else { 1.5 },
                            )
                        }
                        5 => d.gain = (d.gain + delta * 0.5).clamp(0.0, 4.0),
                        6..=8 => d.eq[index - 6] = (d.eq[index - 6] + delta).clamp(0.0, 2.0),
                        _ => {}
                    }
                }
                let direction = f32::from(input.key_down(Key::ArrowRight))
                    - f32::from(input.key_down(Key::ArrowLeft));
                self.session.settings.crossfader =
                    (self.session.settings.crossfader + direction * dt * 0.5).clamp(0.0, 1.0);
                for (k, kind) in [
                    (Key::Z, 0),
                    (Key::X, 1),
                    (Key::C, 2),
                    (Key::V, 3),
                    (Key::B, 4),
                    (Key::N, 5),
                    (Key::M, 6),
                    (Key::Comma, 7),
                ] {
                    if input.key_pressed(k) && self.sample_loading == 0 {
                        self.command(Command::Pad(kind, 0.8));
                    }
                }
            }
            Context::Library => {
                let current = self
                    .selected
                    .and_then(|id| self.tracks.iter().position(|t| t.id == id))
                    .unwrap_or(0);
                let delta = if input.key_pressed(Key::ArrowDown) {
                    1
                } else if input.key_pressed(Key::ArrowUp) {
                    -1
                } else if input.key_pressed(Key::PageDown) {
                    8
                } else if input.key_pressed(Key::PageUp) {
                    -8
                } else {
                    0
                };
                if delta != 0 && !self.tracks.is_empty() {
                    self.selected = Some(
                        self.tracks[(current as isize + delta)
                            .clamp(0, self.tracks.len() as isize - 1)
                            as usize]
                            .id,
                    );
                }
                if input.key_pressed(Key::Enter) {
                    self.edit_track = self.selected_track();
                    if let Some(t) = &self.edit_track {
                        let _ = self.library.requests.send(Request::Details(t.path.clone()));
                    }
                }
            }
            Context::Sequence => {
                if input.key_pressed(Key::ArrowDown) {
                    self.seq_track = (self.seq_track + 1) % 8;
                }
                if input.key_pressed(Key::ArrowUp) {
                    self.seq_track = (self.seq_track + 7) % 8;
                }
                if input.key_pressed(Key::ArrowRight) {
                    self.seq_step = (self.seq_step + 1) % 16;
                }
                if input.key_pressed(Key::ArrowLeft) {
                    self.seq_step = (self.seq_step + 15) % 16;
                }
                if input.key_pressed(Key::OpenBracket) {
                    self.seq_page = self.seq_page.saturating_sub(1);
                }
                if input.key_pressed(Key::CloseBracket) {
                    self.seq_page = (self.seq_page + 1).min(3);
                }
                if input.key_pressed(Key::Enter) {
                    self.toggle_step(self.seq_track, self.seq_page * 16 + self.seq_step);
                }
                if input.key_pressed(Key::Space) {
                    if self.sample_loading == 0 {
                        self.seq_running = !self.seq_running;
                        self.command(Command::SequenceRunning(self.seq_running));
                    } else {
                        self.status = "Poczekaj na przygotowanie banku sampli".into();
                    }
                }
            }
        }
    }
    fn toggle_step(&mut self, track: usize, index: usize) {
        let pattern = self.seq_pattern[track];
        let old = self.session.sequence.patterns[track][pattern].steps[index];
        self.undo.push((track, pattern, index, old));
        if self.undo.len() > 256 {
            self.undo.remove(0);
        }
        let mut value = old;
        value.on = !value.on;
        self.session.sequence.patterns[track][pattern].steps[index] = value;
        self.command(Command::Step {
            track,
            pattern,
            index,
            value,
        });
    }
}

impl App {
    fn browser(&mut self, ui: &mut egui::Ui) {
        panel().inner_margin(6.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal(|ui| {
                ui.strong("BIBLIOTEKA");
                ui.label(RichText::new(format!("{} wyników", self.tracks.len())).color(MUTED));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .id(egui::Id::new("library-search"))
                        .hint_text("Szukaj utworu lub wykonawcy…")
                        .desired_width(230.0),
                );
                if response.has_focus() {
                    self.context = Context::Library;
                }
                if response.changed() {
                    self.refresh();
                }
                let changed = ui.checkbox(&mut self.bpm_filter, "BPM ±4").changed()
                    | ui.checkbox(&mut self.key_filter, "Tonacja").changed()
                    | ui.checkbox(&mut self.unplayed, "Niezagrane").changed();
                if changed {
                    self.refresh();
                }
                if ui.checkbox(&mut self.related, "Pasujące").changed() {
                    self.refresh();
                }
                if ui.small_button("Wyczyść").clicked() {
                    self.search.clear();
                    self.tag.clear();
                    self.key_filter = false;
                    self.bpm_filter = false;
                    self.unplayed = false;
                    self.refresh();
                }
                if ui
                    .button(if self.scanning {
                        "Anuluj skan"
                    } else {
                        "+ Katalog"
                    })
                    .clicked()
                {
                    if self.scanning {
                        self.library.cancel();
                    } else if let Some(path) = rfd::FileDialog::new()
                        .set_title("Wybierz katalog muzyczny")
                        .pick_folder()
                    {
                        self.session.folder = path.to_string_lossy().into_owned();
                        let _ = self.library.requests.send(Request::Scan(path));
                        self.scanning = true;
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Załaduj A  ⌘1").clicked()
                    && let Some(t) = self.selected_track()
                {
                    self.load_track(0, t);
                }
                if ui.button("Załaduj B  ⌘2").clicked()
                    && let Some(t) = self.selected_track()
                {
                    self.load_track(1, t);
                }
                if ui.button("+ Następne").clicked()
                    && let Some(t) = self.selected_track()
                {
                    self.session.queue.push(t);
                }
                if ui.button("CUE Odsłuch").clicked()
                    && let Some(t) = self.selected_track()
                {
                    self.load_track(2, t);
                }
                if ui.small_button("■ Cue").clicked()
                    && let Some(e) = &self.engine
                {
                    e.sources[2].stop();
                }
                if ui.button("Przygotuj / analiza").clicked() {
                    self.edit_track = self.selected_track();
                    if let Some(t) = &self.edit_track {
                        let _ = self.library.requests.send(Request::Details(t.path.clone()));
                    }
                }
                if let Some(track) = self.selected {
                    ui.menu_button("Dodaj do playlisty", |ui| {
                        for (id, name) in &self.playlists {
                            if ui.button(name).clicked() {
                                let _ =
                                    self.library.requests.send(Request::PlaylistAdd(*id, track));
                                ui.close();
                            }
                        }
                    });
                }
                if let Some(playlist) = self.playlist
                    && let Some(track) = self.selected
                {
                    ui.horizontal(|ui| {
                        if ui.small_button("+ Do listy").clicked() {
                            let _ = self
                                .library
                                .requests
                                .send(Request::PlaylistAdd(playlist, track));
                        }
                        if ui.small_button("− Z listy").clicked() {
                            let _ = self
                                .library
                                .requests
                                .send(Request::PlaylistRemove(playlist, track));
                        }
                        for (label, delta) in [("↑", -1), ("↓", 1)] {
                            if ui.small_button(label).clicked() {
                                let _ = self
                                    .library
                                    .requests
                                    .send(Request::PlaylistMove(playlist, track, delta));
                            }
                        }
                    });
                }
                if ui.small_button("Odśwież").clicked() {
                    self.refresh();
                }
                if self.tracks.is_empty() && ui.small_button("Wygeneruj demonstrację").clicked() {
                    self.prepare_demo();
                }
            });
            let width = ui.available_width();
            let height = ui.available_height().max(100.0) - 8.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(128.0, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("library-sidebar")
                            .max_height(height)
                            .show(ui, |ui| {
                                for tag in ["Wszystkie utwory", "Techno", "Trance", "Rave", "Acid"]
                                {
                                    let value = if tag == "Wszystkie utwory" { "" } else { tag };
                                    if ui
                                        .selectable_label(self.tag.eq_ignore_ascii_case(value), tag)
                                        .clicked()
                                    {
                                        self.tag = value.to_owned();
                                        self.refresh();
                                    }
                                }
                                ui.separator();
                                ui.strong("PLAYLISTY");
                                for (id, name) in self.playlists.clone() {
                                    if ui
                                        .selectable_label(self.playlist == Some(id), name)
                                        .clicked()
                                    {
                                        self.playlist = Some(id);
                                        self.refresh();
                                    }
                                }
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.playlist_name)
                                        .hint_text("Nowa playlista")
                                        .desired_width(115.0),
                                );
                                if ui.small_button("Utwórz").clicked() {
                                    let _ = self
                                        .library
                                        .requests
                                        .send(Request::CreatePlaylist(self.playlist_name.clone()));
                                    self.playlist_name.clear();
                                }
                                if let Some(id) = self.playlist {
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Usuń listę").clicked() {
                                            let _ = self
                                                .library
                                                .requests
                                                .send(Request::DeletePlaylist(id));
                                        }
                                        if ui.small_button("Wszystkie").clicked() {
                                            self.playlist = None;
                                            self.refresh();
                                        }
                                    });
                                }
                                if ui.button("Zapisz sesję ⌘S").clicked() {
                                    self.save_session();
                                }
                                if ui.button("Otwórz sesję").clicked()
                                    && let Some(p) = rfd::FileDialog::new()
                                        .add_filter("Sesja JSON", &["json"])
                                        .pick_file()
                                {
                                    match state::load(&p) {
                                        Ok(s) => {
                                            let playing = self.engine.as_ref().is_some_and(|e| {
                                                e.sources.iter().any(|s| {
                                                    s.control.playing.load(Ordering::Relaxed)
                                                })
                                            });
                                            if playing || self.seq_running {
                                                self.status =
                                            "Zatrzymaj decki i rytm przed otwarciem innej sesji."
                                                .into();
                                            } else {
                                                self.session = s;
                                                self.bindings = self.session.bindings.clone();
                                                self.restore_sequence();
                                                for i in 0..2 {
                                                    if let Some(t) = self.session.tracks[i].clone()
                                                    {
                                                        self.load_track(i, t);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => self.status = e.to_string(),
                                    }
                                }
                                if ui.button("Nagrywaj WAV").clicked() {
                                    self.record();
                                }
                            });
                    },
                );
                let table_width = (width - 365.0).max(430.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(table_width, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [54.0, 17.0],
                                egui::Label::new(RichText::new("STATUS").size(9.0).color(MUTED)),
                            );
                            ui.add_sized(
                                [table_width - 270.0, 17.0],
                                egui::Label::new(
                                    RichText::new("WYKONAWCA / TYTUŁ").size(9.0).color(MUTED),
                                ),
                            );
                            for (text, size) in [
                                ("BPM", 42.0),
                                ("TONACJA", 54.0),
                                ("ENERGIA", 42.0),
                                ("CZAS", 42.0),
                            ] {
                                ui.add_sized(
                                    [size, 17.0],
                                    egui::Label::new(RichText::new(text).size(9.0).color(MUTED)),
                                );
                            }
                        });
                        let list = egui::ScrollArea::vertical()
                            .id_salt("track-list")
                            .max_height(height - 20.0)
                            .auto_shrink([false, false])
                            .show_rows(ui, 20.0, self.tracks.len(), |ui, range| {
                                for index in range {
                                    let t = &self.tracks[index];
                                    let id = t.id;
                                    let on_a =
                                        self.session.tracks[0].as_ref().is_some_and(|d| d.id == id);
                                    let on_b =
                                        self.session.tracks[1].as_ref().is_some_and(|d| d.id == id);
                                    let color = if on_a {
                                        AMBER
                                    } else if on_b {
                                        CYAN
                                    } else if t.missing || t.error.is_some() {
                                        Color32::LIGHT_RED
                                    } else {
                                        TEXT
                                    };
                                    let selected = self.selected == Some(id);
                                    let frame = Frame::new().fill(if selected {
                                        Color32::from_rgb(34, 53, 65)
                                    } else if index % 2 == 0 {
                                        Color32::from_rgb(24, 29, 33)
                                    } else {
                                        Color32::TRANSPARENT
                                    });
                                    let response = frame
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.add_sized(
                                                    [54.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new(if t.missing {
                                                            "BRAK"
                                                        } else if on_a {
                                                            "A"
                                                        } else if on_b {
                                                            "B"
                                                        } else if t.played {
                                                            "ZAGRANY"
                                                        } else {
                                                            ""
                                                        })
                                                        .size(9.0)
                                                        .color(color),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [table_width - 270.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new(format!(
                                                            "{}{}{}",
                                                            t.artist,
                                                            if t.artist.is_empty() {
                                                                ""
                                                            } else {
                                                                " — "
                                                            },
                                                            t.title
                                                        ))
                                                        .size(11.0)
                                                        .color(color),
                                                    )
                                                    .truncate(),
                                                );
                                                ui.add_sized(
                                                    [42.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new(
                                                            t.bpm.map_or("—".into(), |b| {
                                                                format!("{b:.1}")
                                                            }),
                                                        )
                                                        .size(10.0)
                                                        .monospace(),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [54.0, 20.0],
                                                    egui::Label::new(
                                                        t.key.as_deref().unwrap_or("—"),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [42.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new(
                                                            "▪".repeat(t.energy as usize),
                                                        )
                                                        .color(GREEN),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [42.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new(time(t.duration))
                                                            .monospace()
                                                            .size(10.0),
                                                    ),
                                                );
                                            });
                                        })
                                        .response;
                                    let response = ui.interact(
                                        response.rect,
                                        ui.id().with(id),
                                        egui::Sense::click(),
                                    );
                                    if response.clicked() {
                                        self.selected = Some(id);
                                        self.context = Context::Library;
                                    }
                                    if response.double_clicked() {
                                        self.edit_track = self.tracks.get(index).cloned();
                                    }
                                }
                            });
                        self.browser_rows = (list.inner_rect.intersect(ui.clip_rect()).height()
                            / (20.0 + ui.spacing().item_spacing.y))
                            .floor() as usize;
                    },
                );
                ui.vertical(|ui| {
                    ui.set_width(200.0);
                    ui.strong("NASTĘPNE");
                    let mut remove = None;
                    for (index, t) in self.session.queue.iter().enumerate().take(8) {
                        ui.horizontal(|ui| {
                            if ui.small_button("×").clicked() {
                                remove = Some(index);
                            }
                            ui.label(
                                RichText::new(format!("{}. {}", index + 1, t.title)).size(11.0),
                            );
                        });
                    }
                    if let Some(i) = remove {
                        self.session.queue.remove(i);
                    }
                    if let Some(t) = self.session.queue.first().cloned() {
                        ui.horizontal(|ui| {
                            if ui.small_button("Pierwszy → A").clicked() {
                                self.load_track(0, t.clone());
                            }
                            if ui.small_button("→ B").clicked() {
                                self.load_track(1, t);
                            }
                        });
                    }
                    if let (Some(reference), Some(candidate)) =
                        (&self.session.tracks[self.active], self.selected_track())
                    {
                        let d = self.session.settings.decks[self.active];
                        ui.label(
                            RichText::new(
                                djejaj_library::music::suggestion(
                                    reference,
                                    &candidate,
                                    self.session.settings.bpm,
                                    d.speed,
                                    d.key_lock,
                                )
                                .1,
                            )
                            .size(10.0)
                            .color(AMBER),
                        );
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(
                            "Ładowanie nie uruchamia muzyki.\nGrające decki są chronione.",
                        )
                        .size(10.0)
                        .color(MUTED),
                    );
                });
            });
        });
    }
    fn record(&mut self) {
        if let Some(e) = &self.engine {
            if e.recorder.active.load(Ordering::Relaxed) {
                e.recorder.stop();
            } else if let Some(path) = rfd::FileDialog::new()
                .set_title("Zapis mastera stereo — nowy plik WAV")
                .set_file_name("DJ-EJAJ-set.wav")
                .add_filter("WAV", &["wav"])
                .save_file()
            {
                let _ = e.recorder.requests.send(RecordRequest::Start(path));
            }
        }
    }
    fn prepare_demo(&mut self) {
        let path = self.dir.join("Demo");
        self.status = "Generowanie oryginalnych utworów demonstracyjnych…".into();
        match djejaj_sampler::export_demo(&path) {
            Ok(()) => {
                self.demo_pending = true;
                let _ = self.library.requests.send(Request::Scan(path));
                self.scanning = true;
            }
            Err(e) => self.status = e.to_string(),
        }
    }
    fn check_readiness(&mut self) {
        let audio = self
            .engine
            .as_ref()
            .is_some_and(|e| !e.shared.error.load(Ordering::Relaxed));
        let missing = self
            .session
            .queue
            .iter()
            .chain(self.session.tracks.iter().flatten())
            .filter(|t| !std::path::Path::new(&t.path).is_file())
            .count();
        let unanalyzed = self
            .session
            .queue
            .iter()
            .filter(|t| t.bpm.is_none() || t.key.is_none())
            .count();
        let cue = self.engine.as_ref().is_some_and(|e| e.channels == 4);
        self.readiness = vec![
            format!(
                "Audio: {}",
                if audio {
                    "gotowe"
                } else {
                    "wybierz sprawne wyjście"
                }
            ),
            format!("Tryb: {}", if self.live { "LIVE" } else { "przygotowanie" }),
            format!("Brakujące pliki setu: {missing}"),
            format!(
                "Kolejka: {} utworów · bez pełnej analizy: {unanalyzed} (możesz miksować ręcznie)",
                self.session.queue.len()
            ),
            format!(
                "Bank sampli: {}",
                if self.sample_loading == 0 {
                    "gotowy"
                } else {
                    "wczytywanie"
                }
            ),
            format!(
                "Oddzielny cue stereo: {}",
                if cue {
                    "włączony"
                } else {
                    "wymaga urządzenia 4-kanałowego"
                }
            ),
        ];
        let folder = self
            .record_path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&self.dir);
        if let Ok(output) = std::process::Command::new("/bin/df")
            .arg("-k")
            .arg(folder)
            .output()
            && output.status.success()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(kib) = text
                .lines()
                .last()
                .and_then(|l| l.split_whitespace().nth(3))
                .and_then(|v| v.parse::<f64>().ok())
            {
                self.readiness.push(format!(
                    "Wolne miejsce na dysku {}: {:.1} GiB",
                    folder.display(),
                    kib / 1_048_576.0
                ));
            }
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_settings {
            let mut open = true;
            egui::Window::new("Ustawienia audio i klawiatury").open(&mut open).default_width(520.0).show(ctx,|ui|{
            egui::ScrollArea::vertical().max_height(600.0).show(ui,|ui|{
            ui.heading("Wyjście stereo");ui.label("Zmiana urządzenia wymaga zatrzymania decków, samplera i nagrania.");egui::ComboBox::from_id_salt("audio-device").selected_text(if self.device.is_empty(){"Domyślne"}else{&self.device}).show_ui(ui,|ui|{ui.selectable_value(&mut self.device,String::new(),"Domyślne");for name in djejaj_audio::output_devices(){ui.selectable_value(&mut self.device,name.clone(),name);}});
            ui.horizontal(|ui|{ui.label("Bufor żądany");for n in [128,256,512]{ui.selectable_value(&mut self.buffer,n,n.to_string());}});ui.checkbox(&mut self.cue_enabled,"4 kanały: master 1–2 / słuchawki 3–4");
            if ui.button("Uruchom / zastosuj audio").clicked(){let busy=self.engine.as_ref().is_some_and(|e|e.shared.voices.load(Ordering::Relaxed)>0||e.sources.iter().any(|s|s.control.playing.load(Ordering::Relaxed))||e.recorder.active.load(Ordering::Relaxed));if busy||self.seq_running||self.sample_loading>0{self.status="Najpierw zatrzymaj wszystkie źródła i nagranie.".into();}else{match Engine::new((!self.device.is_empty()).then_some(self.device.as_str()),self.buffer,self.cue_enabled){Ok(e)=>{self.engine=Some(e);self.restore_sequence();for i in 0..2{if let Some(t)=self.session.tracks[i].clone(){self.load_track(i,t);}}self.status="Wyjście uruchomione; transporty zatrzymane.".into();},Err(e)=>{self.cue_enabled=self.engine.as_ref().is_some_and(|e|e.channels==4);self.status=e.to_string();}}}}
            if let Some(e)=&self.engine{ui.monospace(format!("{} · {} Hz · {} kanały",e.device_name,e.sample_rate,e.channels));ui.label(RichText::new(e.shared.diagnostics().to_string()).small());}
            ui.separator();
            if ui.button("Sprawdź gotowość setu").clicked(){self.check_readiness();}
            for line in &self.readiness{ui.label(line);}
            ui.separator();ui.checkbox(&mut self.live,"LIVE — ochrona grających decków");ui.label("W przygotowaniu Spacja przełącza play/pause. W LIVE zatrzymanie wymaga przytrzymania.");ui.separator();ui.heading("Pary klawiszy regulatorów");
            egui::Grid::new("key-bindings").show(ui,|ui|{for b in &mut self.bindings{ui.label(&b.name);ui.add(egui::TextEdit::singleline(&mut b.less).desired_width(35.0));ui.add(egui::TextEdit::singleline(&mut b.more).desired_width(35.0));ui.end_row();}});
            if let Err(e)=state::validate_bindings(&self.bindings){ui.colored_label(Color32::LIGHT_RED,e.to_string());}
            ui.horizontal(|ui|{if ui.button("Przywróć profil").clicked(){self.bindings=state::default_bindings();}
if ui.button("Zapisz sesję i profil").clicked(){self.save_session();}
if ui.button("Eksport banku WAV").clicked()&& let Some(path)=rfd::FileDialog::new().pick_folder(){match djejaj_sampler::export_bank(&path){Ok(())=>self.status=format!("Bank sampli: {}",path.display()),Err(e)=>self.status=e.to_string()}}});
        });
        });
            self.show_settings = open;
        }
        if self.show_help {
            let mut open = true;
            egui::Window::new("Sampler, sekwencer i skróty").open(&mut open).default_width(540.0).show(ctx,|ui|{
                egui::ScrollArea::vertical().max_height(620.0).show(ui,|ui|{
                    ui.label("⌘L biblioteka · ⌘J granie · ⌘K sekwencer · ⌘F szukaj · ⌘1/2 załaduj A/B");
                    ui.label("Granie: 1/2 deck · Spacja start · Shift+Spacja 700 ms stop · P tap tempo · ⌘. PANIC");
                    ui.label("Pady: Z X C V B N M , · regulatory: pary pod gałkami · Shift: precyzyjnie");
                    ui.label("Sekwencer: strzałki wybierają ścieżkę/krok, Enter przełącza, [ ] wybiera stronę.");
                    let kind=self.seq_track;
                    ui.separator();ui.strong(format!("Sampler: {}",NAMES[kind]));
                    let old=self.session.sample_configs[kind];let config=&mut self.session.sample_configs[kind];
                    ui.horizontal(|ui|{for(mode,label)in [(djejaj_sampler::SampleMode::OneShot,"One-shot"),(djejaj_sampler::SampleMode::Gate,"Gate"),(djejaj_sampler::SampleMode::Loop,"Loop")]{ui.selectable_value(&mut config.mode,mode,label);}});
                    if kind==5{ui.checkbox(&mut config.synth,"Syntezator basowy w sekwencerze");}
                    ui.add(egui::Slider::new(&mut config.volume,0.0..=1.0).text("Głośność pada"));
                    ui.add(egui::Slider::new(&mut config.pan,-1.0..=1.0).text("Panorama"));
                    ui.add(egui::Slider::new(&mut config.pitch,0.5..=2.0).text("Strój / prędkość"));
                    if *config!=old{self.command(Command::SampleConfig(kind,self.session.sample_configs[kind]));}
                    ui.label(self.session.sample_paths[kind].as_deref().unwrap_or("Oryginalne brzmienie DJ EJAJ"));
                    if ui.button("Importuj sample MP3/WAV/FLAC…").clicked(){
                        let busy=self.engine.as_ref().is_some_and(|e|e.shared.voices.load(Ordering::Relaxed)>0||e.sources.iter().any(|s|s.control.playing.load(Ordering::Relaxed)));
                        if self.live||busy||self.seq_running||self.sample_loading>0{self.status="Importuj w trybie przygotowania po zatrzymaniu źródeł.".into();}
                        else if let Some(path)=rfd::FileDialog::new().add_filter("Audio",&["wav","mp3","flac","aiff","ogg","m4a"]).pick_file(){self.queue_sample(kind,path,true);}
                    }
                    ui.separator();
                    let track=self.seq_track;let pattern=self.seq_pattern[track];let index=self.seq_page*16+self.seq_step;
                    ui.strong(format!("{} · wzorzec {} · krok {}",NAMES[track],pattern+1,index+1));
                    let mut step=self.session.sequence.patterns[track][pattern].steps[index];let old=step;
                    ui.checkbox(&mut step.on,"Aktywny");ui.add(egui::Slider::new(&mut step.velocity,0.0..=1.0).text("Velocity"));ui.add(egui::Slider::new(&mut step.gate,0.01..=1.0).text("Gate"));
                    if track==5{
                        ui.add(egui::DragValue::new(&mut step.note).range(24..=84).prefix("Nuta MIDI "));
                        ui.checkbox(&mut step.accent,"Akcent");ui.checkbox(&mut step.slide,"Slide");
                        ui.horizontal(|ui|{
                            let notes=["C","C#","D","Eb","E","F","F#","G","Ab","A","Bb","B"];
                            egui::ComboBox::from_id_salt("scale-root").selected_text(notes[self.session.scale_root as usize]).show_ui(ui,|ui|{for(i,name)in notes.iter().enumerate(){ui.selectable_value(&mut self.session.scale_root,i as u8,*name);}});
                            ui.checkbox(&mut self.session.scale_minor,"moll");
                            if ui.button("Dopasuj do skali").clicked(){step.note=djejaj_library::music::scale_note(step.note,self.session.scale_root,self.session.scale_minor);}
                        });
                    }
                    if step!=old{self.session.sequence.patterns[track][pattern].steps[index]=step;self.command(Command::Step{track,pattern,index,value:step});}
                    let mut length=self.session.sequence.patterns[track][pattern].length;
                    ui.horizontal(|ui|{ui.label("Długość");for n in [16,32,64]{ui.selectable_value(&mut length,n,n.to_string());}});
                    if length!=self.session.sequence.patterns[track][pattern].length{self.session.sequence.patterns[track][pattern].length=length;self.command(Command::Length{track,pattern,length});}
                    let old=self.session.sequence.swing;ui.add(egui::Slider::new(&mut self.session.sequence.swing,0.0..=0.45).text("Swing"));if old!=self.session.sequence.swing{self.command(Command::Swing(self.session.sequence.swing));}
                    ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.scene_select).range(0..=31).prefix("Scena "));if ui.button("Zaplanuj").clicked(){self.command(Command::Scene(self.scene_select));}});
                    ui.text_edit_singleline(&mut self.session.sequence.scenes[self.scene_select].name);
                    ui.add(egui::Slider::new(&mut self.session.settings.sampler_volume,0.0..=1.0).text("Sampler VOL"));
                });
            });
            self.show_help = open;
        }
        if let Some(mut track) = self.edit_track.take() {
            let mut open = true;
            let mut keep = true;
            egui::Window::new("Przygotowanie utworu / działania")
                .open(&mut open)
                .default_width(440.0)
                .show(ctx, |ui| {
                    ui.strong(&track.title);
                    ui.label(&track.path);
                    if let Some(err) = &track.error {
                        ui.colored_label(AMBER, err);
                    }
                    let mut bpm = track.bpm.unwrap_or(self.session.settings.bpm);
                    ui.horizontal(|ui| {
                        ui.label("BPM");
                        if ui
                            .add(
                                egui::DragValue::new(&mut bpm)
                                    .range(30.0..=300.0)
                                    .speed(0.1),
                            )
                            .changed()
                        {
                            track.bpm = Some(bpm);
                        }
                        ui.label("Tonacja");
                        let mut key = track.key.clone().unwrap_or_default();
                        if ui
                            .add(egui::TextEdit::singleline(&mut key).desired_width(65.0))
                            .changed()
                        {
                            track.key = (!key.is_empty()).then_some(key);
                        }
                    });
                    ui.add(egui::Slider::new(&mut track.energy, 1..=5).text("Energia"));
                    ui.horizontal(|ui| {
                        ui.label("Tagi");
                        ui.text_edit_singleline(&mut track.tags);
                    });
                    ui.add(
                        egui::DragValue::new(&mut track.downbeat)
                            .range(0.0..=track.duration.max(1.0))
                            .speed(0.01)
                            .prefix("Downbeat [s] "),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Załaduj A").clicked() {
                            self.load_track(0, track.clone());
                        }
                        if ui.button("Załaduj B").clicked() {
                            self.load_track(1, track.clone());
                        }
                        if ui.button("Dodaj do Następne").clicked() {
                            self.session.queue.push(track.clone());
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Analizuj w tle").clicked() {
                            let _ = self.library.requests.send(Request::Analyze(track.id));
                            self.status =
                                "Analiza: waveform, BPM i tonacja. Możesz kontynuować granie."
                                    .into();
                            keep = false;
                        }
                        if ui.button("Zapisz zmiany").clicked() {
                            let _ = self
                                .library
                                .requests
                                .send(Request::Save(Box::new(track.clone())));
                            for t in self.session.tracks.iter_mut().flatten() {
                                if t.id == track.id {
                                    *t = track.clone();
                                }
                            }
                            keep = false;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Początek frazy [s]");
                        ui.add(
                            egui::DragValue::new(&mut track.phrase_start)
                                .speed(0.01)
                                .range(0.0..=track.duration.max(1.0)),
                        );
                        for bars in [8, 16, 32] {
                            ui.selectable_value(
                                &mut track.phrase_bars,
                                bars,
                                format!("{bars} taktów"),
                            );
                        }
                    });
                    for (name, seconds) in &mut track.markers {
                        ui.horizontal(|ui| {
                            ui.label(name.as_str());
                            ui.add(
                                egui::DragValue::new(seconds)
                                    .speed(0.01)
                                    .range(0.0..=track.duration.max(1.0)),
                            );
                        });
                    }
                    if track.markers.len() < 32 {
                        ui.horizontal(|ui| {
                            for name in ["INTRO", "BREAK", "BUILD", "DROP", "OUTRO"] {
                                if ui.small_button(format!("+ {name}")).clicked() {
                                    track.markers.push((name.to_owned(), 0.0));
                                }
                            }
                        });
                    }
                    if let Some(a) = &track.analysis {
                        ui.label(format!(
                            "Pewność BPM: {:.0}% · tonacji: {:.0}%",
                            a.bpm_confidence * 100.0,
                            a.key_confidence * 100.0
                        ));
                    }
                    ui.label(
                        "Analiza jest wskazówką. Ustaw ręcznie BPM/downbeat i sprawdź odsłuch.",
                    );
                });
            if open && keep {
                self.edit_track = Some(track);
            }
        }
    }
}

impl App {
    fn draw(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(24, 28, 33))
                    .inner_margin(10.0),
            )
            .show(ctx, |ui| self.header(ui));
        egui::TopBottomPanel::bottom("status")
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(22, 27, 31))
                    .inner_margin(7.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if self.live { "LIVE" } else { "PRZYGOTOWANIE" })
                            .color(GREEN),
                    );
                    ui.add(egui::Label::new(RichText::new(&self.status).size(11.0)).truncate())
                        .on_hover_text(&self.status);
                    if let Some(e) = &self.engine
                        && e.recorder.active.load(Ordering::Relaxed)
                    {
                        ui.colored_label(Color32::LIGHT_RED, "REC");
                        if ui.small_button("Stop REC").clicked() {
                            e.recorder.stop();
                        }
                    }
                });
                if ctx.content_rect().height() > 880.0 {
                    ui.label(
                        RichText::new(
                            "⌘L Biblioteka · ⌘J Granie · ⌘K Sekwencer · ⌘S Zapisz · ⌘. PANIC",
                        )
                        .size(10.0)
                        .color(MUTED),
                    );
                }
            });
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(17, 21, 25))
                    .inner_margin(8.0),
            )
            .show(ctx, |ui| {
                let base = ui.cursor().min.y;
                self.layout_metrics[4] = ui.available_height();
                let width = ui.available_width();
                let mixer_width = 290.0;
                let deck_width = (width - mixer_width - 16.0) * 0.5;
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(deck_width, 200.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_max_width(deck_width);
                            self.deck(ui, 0);
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(mixer_width, 200.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_max_width(mixer_width);
                            self.mixer(ui);
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(deck_width, 200.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_max_width(deck_width);
                            self.deck(ui, 1);
                        },
                    );
                });
                self.layout_metrics[0] = ui.cursor().min.y - base;
                self.effects(ui);
                self.layout_metrics[1] = ui.cursor().min.y - base - self.layout_metrics[0];
                self.sequence(ui);
                self.layout_metrics[2] =
                    ui.cursor().min.y - base - self.layout_metrics[0] - self.layout_metrics[1];
                self.layout_metrics[3] = ui.available_height();
                self.browser(ui);
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.frames += 1;
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if self.frames == 2 && self.demo_pending && self.tracks.is_empty() {
            self.prepare_demo();
        }
        self.library.pause_analysis(
            self.live
                && (self.seq_running
                    || self.engine.as_ref().is_some_and(|e| {
                        e.sources[..2]
                            .iter()
                            .any(|s| s.control.playing.load(Ordering::Relaxed))
                    })),
        );
        self.poll();
        if let Some(e) = &self.engine {
            let scene = e.shared.active_scene.load(Ordering::Relaxed) as usize;
            if scene != self.last_scene {
                self.seq_pattern = self.session.sequence.scenes[scene].patterns;
                self.last_scene = scene;
            }
        }
        if let Some(e) = &self.engine {
            while let Some(block) = e.shared.visual.pop() {
                self.spectrum.feed(&block);
            }
        }
        self.keys(ctx, dt);
        for i in 0..2 {
            if let (Some(e), Some(t)) = (&self.engine, &mut self.session.tracks[i]) {
                let d = self.session.settings.decks[i];
                let cross = if self.session.settings.thru {
                    1.0
                } else {
                    djejaj_dsp::crossfade(self.session.settings.crossfader)[i]
                };
                if e.sources[i].control.playing.load(Ordering::Relaxed)
                    && d.fader * cross * d.gain * self.session.settings.master > 0.01
                    && e.shared.deck_meter[i].get() > 0.001
                {
                    self.played[i] += dt as f64;
                    if self.played[i] >= 30.0 && !t.played {
                        t.played = true;
                        let _ = self
                            .library
                            .requests
                            .send(Request::Played(t.id, t.path.clone()));
                    }
                }
            }
        }
        self.draw(ctx);
        self.dialogs(ctx);
        if let Some(e) = &self.engine
            && let Err(err) = e.settings(self.session.settings.clone())
        {
            self.status = err.to_string();
        }
        if self.frames == 120 && self.screenshot.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(path) = self.screenshot.take()
            {
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Err(e) = image::save_buffer(
                    &path,
                    &rgba,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    self.status = e.to_string();
                } else {
                    self.status = format!("Zrzut zapisany: {}", path.display());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    fn on_exit(&mut self) {
        if self.session_valid {
            self.session.bindings = self.bindings.clone();
            let _ = state::save(&self.dir.join("session.json"), &self.session);
        }
        if let Some(e) = &self.engine {
            e.shared.panic();
            e.recorder.stop();
        }
    }
}

impl App {
    fn tap_tempo(&mut self) {
        let now = Instant::now();
        if self
            .taps
            .back()
            .is_some_and(|t| now.duration_since(*t).as_secs_f32() > 2.0)
        {
            self.taps.clear();
        }
        self.taps.push_back(now);
        while self.taps.len() > 5 {
            self.taps.pop_front();
        }
        if self.taps.len() >= 2 {
            let elapsed =
                now.duration_since(self.taps[0]).as_secs_f32() / (self.taps.len() - 1) as f32;
            let bpm = 60.0 / elapsed;
            if (60.0..=220.0).contains(&bpm) {
                self.session.settings.bpm = bpm;
            }
        }
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("DJ EJAJ").size(23.0).strong());
                ui.label(RichText::new("by NT4.pl").size(10.0).color(MUTED));
            });
            ui.separator();
            ui.label(
                RichText::new(if self.live { "LIVE" } else { "PRZYGOTOWANIE" })
                    .strong()
                    .color(if self.live { GREEN } else { AMBER }),
            );
            ui.separator();
            ui.add(
                egui::DragValue::new(&mut self.session.settings.bpm)
                    .range(60.0..=220.0)
                    .speed(0.1)
                    .fixed_decimals(2)
                    .suffix(" BPM"),
            );
            if ui.small_button("TAP").clicked() {
                self.tap_tempo();
            }
            ui.label("4/4");
            let beat = self.engine.as_ref().map_or(0.0, |e| e.shared.beat());
            ui.label(
                RichText::new(format!("TAKT {:02} / 16", (beat / 4.0) as usize % 16 + 1))
                    .monospace(),
            );
            ui.separator();
            if let Some(e) = &self.engine {
                ui.label(
                    RichText::new("AUDIO").color(if e.shared.error.load(Ordering::Relaxed) {
                        Color32::LIGHT_RED
                    } else {
                        GREEN
                    }),
                );
                ui.label(
                    RichText::new(format!(
                        "{} kHz · żąd. {}",
                        e.sample_rate / 1000,
                        e.buffer_requested
                    ))
                    .small()
                    .color(MUTED),
                );
            } else {
                ui.colored_label(AMBER, "AUDIO OFF");
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .button(RichText::new("PANIC").color(Color32::LIGHT_RED))
                    .clicked()
                {
                    self.panic();
                }
                if ui.button("Ustawienia").clicked() {
                    self.show_settings = true;
                }
                if ui.button("?").clicked() {
                    self.show_help = true;
                }
                for (c, name) in [
                    (Context::Sequence, "Sekwencer"),
                    (Context::Library, "Biblioteka"),
                    (Context::Play, "Granie"),
                ] {
                    if ui.selectable_label(self.context == c, name).clicked() {
                        self.context = c;
                        if c == Context::Sequence {
                            self.seq_open = true;
                        }
                    }
                }
            });
        });
    }
    fn deck(&mut self, ui: &mut egui::Ui, i: usize) {
        let color = if i == 0 { AMBER } else { CYAN };
        let track = self.session.tracks[i].clone();
        let (position, playing, ready) = self.engine.as_ref().map_or((0.0, false, false), |e| {
            (
                e.sources[i].position(),
                e.sources[i].control.playing.load(Ordering::Relaxed),
                e.sources[i].control.ready.load(Ordering::Relaxed),
            )
        });
        panel()
            .stroke(Stroke::new(
                if self.active == i { 1.5_f32 } else { 0.8_f32 },
                color.gamma_multiply(if self.active == i { 1.0 } else { 0.5 }),
            ))
            .show(ui, |ui| {
                ui.spacing_mut().button_padding.y = 2.0;
                ui.spacing_mut().interact_size.y = 18.0;
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.active == i,
                            RichText::new(format!(
                                "{}  DECK {}",
                                i + 1,
                                if i == 0 { "A" } else { "B" }
                            ))
                            .strong()
                            .color(color),
                        )
                        .clicked()
                    {
                        self.active = i;
                        self.context = Context::Play;
                        self.keys_blocked = true;
                    }
                    ui.label(
                        RichText::new(if playing {
                            "▶ GRA · CHRONIONY"
                        } else if self.pending[i].is_some() {
                            "PRZYGOTOWYWANIE"
                        } else if ready {
                            "GOTOWY"
                        } else {
                            "PUSTY"
                        })
                        .size(10.0)
                        .color(if playing { GREEN } else { MUTED }),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.monospace(format!(
                            "−{}",
                            time(track.as_ref().map_or(0.0, |t| t.duration) - position)
                        ));
                    });
                });
                ui.label(
                    RichText::new(track.as_ref().map_or(
                        "Wybierz utwór w bibliotece".to_owned(),
                        |t| {
                            format!(
                                "{}{}{}",
                                t.artist,
                                if t.artist.is_empty() { "" } else { " — " },
                                t.title
                            )
                        },
                    ))
                    .strong()
                    .size(13.0),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(
                            track
                                .as_ref()
                                .and_then(|t| t.bpm)
                                .map_or("BPM —".into(), |b| {
                                    format!("{:.1} BPM", b * self.session.settings.decks[i].speed)
                                }),
                        )
                        .monospace()
                        .color(color),
                    );
                    ui.label(
                        track
                            .as_ref()
                            .and_then(|t| t.key.clone())
                            .unwrap_or_else(|| "Tonacja —".into()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} / {}",
                            time(position),
                            time(track.as_ref().map_or(0.0, |t| t.duration))
                        ))
                        .small()
                        .color(MUTED),
                    );
                });
                let peaks = track
                    .as_ref()
                    .and_then(|t| t.analysis.as_ref())
                    .map_or(&[][..], |a| a.waveform.as_slice());
                let response = waveform(
                    ui,
                    peaks,
                    position,
                    track.as_ref().map_or(0.0, |t| t.duration),
                    color,
                    track.as_ref().and_then(|t| t.bpm),
                    track.as_ref().map_or(0.0, |t| t.downbeat),
                );
                if let Some(t) = &track
                    && t.duration > 0.0
                {
                    for (name, seconds) in &t.markers {
                        let x = response.rect.left()
                            + (*seconds / t.duration).clamp(0.0, 1.0) as f32
                                * response.rect.width();
                        ui.painter().line_segment(
                            [
                                egui::pos2(x, response.rect.top()),
                                egui::pos2(x, response.rect.bottom()),
                            ],
                            Stroke::new(1.0_f32, AMBER),
                        );
                        ui.painter().text(
                            egui::pos2(x + 2.0, response.rect.top() + 1.0),
                            egui::Align2::LEFT_TOP,
                            name,
                            FontId::proportional(8.0),
                            AMBER,
                        );
                    }
                }
                if response.clicked()
                    && let (Some(pos), Some(t), Some(e)) = (
                        response.interact_pointer_pos(),
                        track.as_ref(),
                        self.engine.as_ref(),
                    )
                {
                    e.sources[i].seek(
                        ((pos.x - response.rect.left()) / response.rect.width()) as f64
                            * t.duration,
                    );
                }
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new(if playing { "▶ GRA" } else { "▶ START" }).color(color),
                        )
                        .clicked()
                    {
                        self.start(i);
                    }
                    let stop = ui.button("■ STOP").on_hover_text(if self.live {
                        "Przytrzymaj 700 ms, aby zatrzymać deck"
                    } else {
                        "Zatrzymaj"
                    });
                    if stop.is_pointer_button_down_on() {
                        let since = self.pointer_stop[i].get_or_insert_with(Instant::now);
                        if (!self.live || since.elapsed().as_millis() >= 700)
                            && let Some(e) = &self.engine
                        {
                            e.sources[i].stop();
                        }
                    } else {
                        self.pointer_stop[i] = None;
                    }
                    let progress = self.pointer_stop[i].or(if i == self.active {
                        self.stop_since
                    } else {
                        None
                    });
                    if let Some(since) = progress {
                        let fraction = (since.elapsed().as_secs_f32() / 0.7).min(1.0);
                        let r = egui::Rect::from_min_size(
                            stop.rect.left_bottom(),
                            Vec2::new(stop.rect.width() * fraction, 2.0),
                        );
                        ui.painter().rect_filled(r, 0.0, AMBER);
                    }
                    if ui
                        .button("CUE ↩")
                        .on_hover_text(
                            "Powrót do pierwszego cue; w LIVE tylko na zatrzymanym decku",
                        )
                        .clicked()
                    {
                        if playing && self.live {
                            self.status = "Zatrzymaj deck przed powrotem do CUE.".into();
                        } else if let Some(e) = &self.engine {
                            e.sources[i].seek(track.as_ref().map_or(0.0, |t| t.cue[0]));
                        }
                    }
                    if ui
                        .button("TEMPO SYNC")
                        .on_hover_text("Dopasuj tempo do globalnego BPM; fazę ustaw ręcznie")
                        .clicked()
                    {
                        if let Some(bpm) = track.as_ref().and_then(|t| t.bpm) {
                            self.session.settings.decks[i].speed =
                                (self.session.settings.bpm / bpm).clamp(0.8, 1.2);
                        } else {
                            self.status = "Ustaw BPM utworu przed synchronizacją.".into();
                        }
                    }
                    let d = &mut self.session.settings.decks[i];
                    if ui.checkbox(&mut d.key_lock, "KEY LOCK").changed() {
                        d.speed = d.speed.clamp(0.8, 1.2);
                    }
                    ui.checkbox(&mut d.cue, "CUE")
                        .on_hover_text("Cue na kanałach 3–4; wymaga wyjścia czterokanałowego");
                });
                ui.horizontal(|ui| {
                    ui.label("TEMPO");
                    let d = &mut self.session.settings.decks[i];
                    ui.add(
                        egui::Slider::new(
                            &mut d.speed,
                            if d.key_lock { 0.8..=1.2 } else { 0.5..=1.5 },
                        )
                        .fixed_decimals(3)
                        .show_value(true),
                    );
                    ui.label(
                        RichText::new(if self.active == i {
                            "T − / G +"
                        } else {
                            "aktywuj 2"
                        })
                        .small()
                        .color(MUTED),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("HOT CUE");
                    for n in 0..4 {
                        if ui
                            .button(format!("{}", n + 1))
                            .on_hover_text("Kliknij: skok · Shift: zapisz aktualną pozycję")
                            .clicked()
                        {
                            if ui.input(|i| i.modifiers.shift) {
                                if let Some(t) = self.session.tracks[i].as_mut() {
                                    t.cue[n] = position;
                                    let _ = self
                                        .library
                                        .requests
                                        .send(Request::Save(Box::new(t.clone())));
                                }
                            } else if let Some(e) = &self.engine {
                                e.sources[i].seek(track.as_ref().map_or(0.0, |t| t.cue[n]));
                            }
                        }
                    }
                    ui.separator();
                    ui.label("LOOP");
                    for beats in [1, 2, 4, 8, 16] {
                        if ui.small_button(beats.to_string()).clicked()
                            && let Some(e) = &self.engine
                        {
                            let bpm = track
                                .as_ref()
                                .and_then(|t| t.bpm)
                                .unwrap_or(self.session.settings.bpm);
                            let length = 60.0 / bpm as f64 * beats as f64;
                            e.sources[i].set_loop(position, position + length);
                        }
                    }
                    if ui.small_button("OFF").clicked()
                        && let Some(e) = &self.engine
                    {
                        e.sources[i].set_loop(0.0, 0.0);
                    }
                });
            });
    }
    fn mixer(&mut self, ui: &mut egui::Ui) {
        panel().show(ui, |ui| {
            ui.spacing_mut().button_padding.y = 2.0;
            ui.spacing_mut().interact_size.y = 18.0;
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.horizontal(|ui| {
                ui.strong("MIKSER");
                let is_playing = self.engine.as_ref().is_some_and(|e| {
                    e.sources[..2]
                        .iter()
                        .any(|s| s.control.playing.load(Ordering::Relaxed))
                });
                ui.add_enabled_ui(!self.live || !is_playing, |ui| {
                    ui.checkbox(&mut self.session.settings.thru, "THRU");
                });
            });
            ui.columns(2, |cols| {
                for (i, ui) in cols.iter_mut().enumerate() {
                    let color = if i == 0 { AMBER } else { CYAN };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, if i == 0 { "A" } else { "B" });
                        ui.add(
                            egui::DragValue::new(&mut self.session.settings.decks[i].gain)
                                .range(0.0..=4.0)
                                .speed(0.01)
                                .prefix("GAIN ")
                                .fixed_decimals(2),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let d = &mut self.session.settings.decks[i];
                        for (j, label) in ["LOW", "MID", "HIGH"].iter().enumerate() {
                            let b = &self.bindings[j + 6];
                            let keys = if i == self.active {
                                format!("{}/{}", b.less, b.more)
                            } else {
                                "—".into()
                            };
                            mini_knob(ui, label, &mut d.eq[j], &keys, color);
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut self.session.settings.decks[i].fader, 0.0..=1.0)
                            .text("VOL")
                            .show_value(false),
                    );
                }
            });
            ui.horizontal(|ui| {
                let levels = self.engine.as_ref().map_or([0.0; 2], |e| {
                    [e.shared.meter[0].get(), e.shared.meter[1].get()]
                });
                meter(ui, levels);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("MASTER");
                        ui.add(
                            egui::Slider::new(&mut self.session.settings.master, 0.0..=1.0)
                                .show_value(false),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.colored_label(AMBER, "A");
                        ui.add_enabled(
                            !self.session.settings.thru,
                            egui::Slider::new(&mut self.session.settings.crossfader, 0.0..=1.0)
                                .show_value(false),
                        );
                        ui.colored_label(CYAN, "B");
                    });
                });
            });
        });
    }
    fn effects(&mut self, ui: &mut egui::Ui) {
        panel().inner_margin(5.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                let color = if self.active == 0 { AMBER } else { CYAN };
                ui.vertical(|ui| {
                    ui.strong(format!(
                        "FX · DECK {}",
                        if self.active == 0 { "A" } else { "B" }
                    ));
                    ui.checkbox(
                        &mut self.session.settings.decks[self.active].filter_on,
                        "FILTER ON",
                    );
                    ui.checkbox(
                        &mut self.session.settings.decks[self.active].highpass,
                        "HIGH PASS",
                    );
                });
                for n in 0..4 {
                    let d = &self.session.settings.decks[self.active];
                    let (mut value, range, display, label, log) = match n {
                        0 => (
                            d.cutoff,
                            25.0..=20000.0,
                            if d.cutoff >= 1000.0 {
                                format!("{:.1} kHz", d.cutoff / 1000.0)
                            } else {
                                format!("{:.0} Hz", d.cutoff)
                            },
                            "FILTER",
                            true,
                        ),
                        1 => (
                            d.delay,
                            0.0..=1.0,
                            format!("{:.0}%", d.delay * 100.0),
                            "DELAY",
                            false,
                        ),
                        2 => (
                            d.reverb,
                            0.0..=1.0,
                            format!("{:.0}%", d.reverb * 100.0),
                            "REVERB",
                            false,
                        ),
                        _ => (
                            d.drive,
                            0.0..=1.0,
                            format!("{:.0}%", d.drive * 100.0),
                            "DRIVE",
                            false,
                        ),
                    };
                    let b = &self.bindings[n];
                    let keys = format!("{} − / {} +", b.less, b.more);
                    if knob(
                        ui,
                        &mut value,
                        range,
                        KnobStyle {
                            label,
                            display: &display,
                            keys: &keys,
                            color,
                            log,
                        },
                    )
                    .changed()
                    {
                        let d = &mut self.session.settings.decks[self.active];
                        match n {
                            0 => {
                                d.cutoff = value;
                                d.filter_on = true;
                            }
                            1 => d.delay = value,
                            2 => d.reverb = value,
                            _ => d.drive = value,
                        }
                    }
                }
                ui.separator();
                ui.vertical(|ui| {
                    ui.label("DELAY SYNC");
                    egui::ComboBox::from_id_salt("delay-time")
                        .selected_text(format!("{} beat", self.session.settings.delay_beats))
                        .width(90.0)
                        .show_ui(ui, |ui| {
                            for beats in [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0] {
                                ui.selectable_value(
                                    &mut self.session.settings.delay_beats,
                                    beats,
                                    format!("{beats} beat"),
                                );
                            }
                        });
                    ui.add(
                        egui::Slider::new(&mut self.session.settings.feedback, 0.0..=0.9)
                            .text("FB")
                            .show_value(false),
                    );
                });
                ui.vertical(|ui| {
                    ui.label("PRESET");
                    for (name, delay, reverb, drive) in [
                        ("Przejście", 0.25, 0.1, 0.08),
                        ("Przestrzeń", 0.15, 0.4, 0.0),
                        ("Acid", 0.12, 0.08, 0.35),
                    ] {
                        if ui.small_button(name).clicked() {
                            let d = &mut self.session.settings.decks[self.active];
                            d.delay = delay;
                            d.reverb = reverb;
                            d.drive = drive;
                        }
                    }
                });
                ui.vertical(|ui| {
                    let d = &mut self.session.settings.decks[self.active];
                    ui.checkbox(&mut d.flanger, "FLANGER");
                    ui.add(
                        egui::Slider::new(&mut d.modulation, 0.0..=1.0)
                            .text("MOD")
                            .show_value(false),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.session.settings.decay, 0.0..=0.88)
                            .text("DECAY")
                            .show_value(false),
                    );
                });
                ui.separator();
                self.spectrum.draw(
                    ui,
                    self.engine
                        .as_ref()
                        .map_or(48000.0, |e| e.sample_rate as f32),
                );
            });
        });
    }
    fn sequence(&mut self, ui: &mut egui::Ui) {
        panel().inner_margin(5.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 2.0);
            ui.spacing_mut().button_padding = Vec2::new(4.0, 0.0);
            ui.spacing_mut().interact_size.y = 14.0;
            ui.horizontal(|ui| {
                if ui.selectable_label(self.seq_open, "SEKWENCER").clicked() {
                    self.seq_open = !self.seq_open;
                }
                if ui
                    .button(if self.seq_running {
                        "■ STOP RYTM"
                    } else {
                        "▶ START RYTM"
                    })
                    .clicked()
                {
                    if self.sample_loading == 0 {
                        self.seq_running = !self.seq_running;
                        self.command(Command::SequenceRunning(self.seq_running));
                    } else {
                        self.status = "Poczekaj na przygotowanie banku sampli".into();
                    }
                }
                ui.label("64 kroki · 8 ścieżek");
                for page in 0..4 {
                    if ui
                        .selectable_label(
                            self.seq_page == page,
                            format!("{}–{}", page * 16 + 1, page * 16 + 16),
                        )
                        .clicked()
                    {
                        self.seq_page = page;
                        self.context = Context::Sequence;
                    }
                }
                if ui.small_button("Cofnij").clicked()
                    && let Some((track, pattern, index, value)) = self.undo.pop()
                {
                    self.session.sequence.patterns[track][pattern].steps[index] = value;
                    self.command(Command::Step {
                        track,
                        pattern,
                        index,
                        value,
                    });
                }
                ui.label("Zmiany scen: 1 TAKT");
            });
            if !self.seq_open {
                return;
            }
            let scene = self.engine.as_ref().map_or(1, |e| {
                e.shared.active_scene.load(Ordering::Relaxed) as usize
            });
            let pending = self
                .engine
                .as_ref()
                .map_or(u64::MAX, |e| e.shared.pending_scene.load(Ordering::Relaxed));
            let step = self
                .engine
                .as_ref()
                .map_or(0, |e| e.shared.step.load(Ordering::Relaxed) as usize);
            let width = ui.available_width();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new((width - 205.0).max(500.0), 152.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        let cell = ((ui.available_width() - 185.0) / 16.0 - 3.0).max(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(176.0);
                            for i in 0..16 {
                                ui.add_sized(
                                    [cell, 13.0],
                                    egui::Label::new(
                                        RichText::new((i + 1 + self.seq_page * 16).to_string())
                                            .size(9.0)
                                            .color(MUTED),
                                    ),
                                );
                            }
                        });
                        for track in 0..8 {
                            ui.horizontal(|ui| {
                                let color = if track == 5 { CYAN } else { AMBER };
                                if ui
                                    .add_sized(
                                        [100.0, 15.0],
                                        egui::Button::new(
                                            RichText::new(format!(
                                                "{} [{}]",
                                                NAMES[track], KEYS[track]
                                            ))
                                            .size(10.0),
                                        )
                                        .selected(self.seq_track == track),
                                    )
                                    .clicked()
                                {
                                    self.seq_track = track;
                                    self.context = Context::Sequence;
                                }
                                if ui
                                    .add_sized(
                                        [19.0, 15.0],
                                        egui::Button::new("M").selected(
                                            self.session.sequence.scenes[scene].muted[track],
                                        ),
                                    )
                                    .clicked()
                                {
                                    let muted = !self.session.sequence.scenes[scene].muted[track];
                                    self.session.sequence.scenes[scene].muted[track] = muted;
                                    self.command(Command::Mute(track, muted));
                                }
                                let mut pattern = self.seq_pattern[track];
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut pattern)
                                            .range(0..=7)
                                            .prefix("P")
                                            .speed(0.1),
                                    )
                                    .changed()
                                {
                                    self.seq_pattern[track] = pattern;
                                    self.session.sequence.scenes[scene].patterns[track] = pattern;
                                    self.command(Command::ScenePattern {
                                        scene,
                                        track,
                                        pattern,
                                    });
                                }
                                for s in 0..16 {
                                    let index = self.seq_page * 16 + s;
                                    let value = self.session.sequence.patterns[track]
                                        [self.seq_pattern[track]]
                                        .steps[index];
                                    let focused = self.context == Context::Sequence
                                        && self.seq_track == track
                                        && self.seq_step == s;
                                    let fill = if value.on {
                                        color.gamma_multiply(if step == index { 1.0 } else { 0.65 })
                                    } else {
                                        Color32::from_rgb(40, 46, 52)
                                    };
                                    let button = egui::Button::new(if track == 5 && value.on {
                                        format!("{}", value.note)
                                    } else {
                                        String::new()
                                    })
                                    .fill(fill)
                                    .stroke(Stroke::new(
                                        if focused { 1.5_f32 } else { 0.5_f32 },
                                        if focused {
                                            TEXT
                                        } else {
                                            Color32::from_gray(55)
                                        },
                                    ));
                                    let response = ui.add_sized([cell, 15.0], button);
                                    if response.clicked() {
                                        self.seq_track = track;
                                        self.seq_step = s;
                                        self.context = Context::Sequence;
                                        self.toggle_step(track, index);
                                    }
                                    if response.secondary_clicked() {
                                        self.seq_track = track;
                                        self.seq_step = s;
                                        self.context = Context::Sequence;
                                        self.show_help = true;
                                    }
                                }
                            });
                        }
                    },
                );
                ui.vertical(|ui| {
                    ui.set_width(190.0);
                    ui.strong("SCENY");
                    egui::ScrollArea::vertical()
                        .id_salt("scene-list")
                        .max_height(132.0)
                        .show(ui, |ui| {
                            for n in 0..32 {
                                let label = format!(
                                    "{:02} {}{}",
                                    n + 1,
                                    self.session.sequence.scenes[n].name,
                                    if pending == n as u64 {
                                        " · CZEKA"
                                    } else if scene == n {
                                        " · GRA"
                                    } else {
                                        ""
                                    }
                                );
                                let button = egui::Button::new(RichText::new(label).color(
                                    if pending == n as u64 {
                                        AMBER
                                    } else if scene == n {
                                        GREEN
                                    } else {
                                        TEXT
                                    },
                                ))
                                .selected(scene == n);
                                if ui.add_sized([185.0, 22.0], button).clicked() {
                                    self.command(Command::Scene(n));
                                }
                            }
                        });
                    ui.horizontal(|ui| {
                        if ui.small_button("Anuluj").clicked() {
                            self.command(Command::CancelScene);
                        }
                        ui.label(format!("Aktywna: {}", scene + 1));
                    });
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (egui::Context, App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        let mut app = App::init(&ctx, false, None, dir.path().to_path_buf(), false).unwrap();
        app.tracks = (0..100)
            .map(|id| Track {
                id,
                title: format!("Track {id}"),
                phrase_bars: 16,
                ..Track::default()
            })
            .collect();
        (ctx, app, dir)
    }
    fn key(k: Key, pressed: bool) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: Some(k),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280.0, 800.0),
            )),
            focused: true,
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            app.keys(ctx, 0.016);
            app.draw(ctx);
        });
    }
    #[test]
    fn laptop_layout_keeps_library_visible_with_and_without_sequencer() {
        let (ctx, mut app, _dir) = setup();
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        assert!(
            app.browser_rows >= 4,
            "expanded sequencer: {} visible rows; layout {:?}",
            app.browser_rows,
            app.layout_metrics
        );
        app.seq_open = false;
        frame(&ctx, &mut app, vec![]);
        assert!(
            app.browser_rows >= 8,
            "collapsed sequencer: {} visible rows",
            app.browser_rows
        );
    }
    #[test]
    fn held_knob_key_does_not_jump_to_new_deck() {
        let (ctx, mut app, _dir) = setup();
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(Key::Q, true)]);
        assert!(app.session.settings.decks[0].cutoff < 20000.0);
        frame(&ctx, &mut app, vec![key(Key::Num2, true)]);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.session.settings.decks[1].cutoff, 20000.0);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Q, false), key(Key::Num2, false)],
        );
        frame(&ctx, &mut app, vec![key(Key::Q, true)]);
        assert!(app.session.settings.decks[1].cutoff < 20000.0);
    }
    #[test]
    fn search_accepts_space_and_letters_without_turning_knobs() {
        let (ctx, mut app, _dir) = setup();
        app.context = Context::Library;
        frame(&ctx, &mut app, vec![]);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("library-search")));
        frame(&ctx, &mut app, vec![]);
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Q, true), egui::Event::Text("q".into())],
        );
        frame(
            &ctx,
            &mut app,
            vec![key(Key::Space, true), egui::Event::Text(" ".into())],
        );
        assert_eq!(app.search, "q ");
        assert_eq!(app.session.settings.decks[0].cutoff, 20000.0);
    }
}
