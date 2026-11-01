pub mod analysis;
mod decoder;
pub mod music;
use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
pub use decoder::Decoder;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub sample_rate: u32,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub energy: u8,
    pub tags: String,
    pub missing: bool,
    pub error: Option<String>,
    pub analysis: Option<analysis::Analysis>,
    pub cue: [f64; 4],
    pub downbeat: f64,
    pub played: bool,
    #[serde(default)]
    pub markers: Vec<(String, f64)>,
    #[serde(default)]
    pub phrase_start: f64,
    #[serde(default = "default_phrase_bars")]
    pub phrase_bars: usize,
}
fn default_phrase_bars() -> usize {
    16
}

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub text: String,
    pub bpm_min: Option<f32>,
    pub bpm_max: Option<f32>,
    pub key: Option<String>,
    pub unplayed: bool,
    pub tag: String,
    pub playlist: Option<i64>,
}
pub enum Request {
    Scan(PathBuf),
    Cancel,
    Query(u64, Query),
    Analyze(i64),
    Details(String),
    Playlists,
    CreatePlaylist(String),
    DeletePlaylist(i64),
    PlaylistAdd(i64, i64),
    PlaylistRemove(i64, i64),
    PlaylistMove(i64, i64, i32),
    Save(Box<Track>),
    Played(i64, String),
    Shutdown,
}
pub enum Event {
    Rows(u64, Vec<Track>),
    Progress(usize),
    Scanned(usize),
    Analyzed(Box<Track>),
    Detailed(Box<Track>),
    Playlists(Vec<(i64, String)>),
    Error(String),
    Saved,
}
pub struct Library {
    pub requests: Sender<Request>,
    pub events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}
impl Library {
    pub fn open(path: PathBuf) -> Result<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let conn = open_db(&path)?;
        let (tx, rx) = crossbeam_channel::unbounded();
        let (etx, erx) = crossbeam_channel::unbounded();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let paused = Arc::new(AtomicBool::new(false));
        let analysis_paused = paused.clone();
        let scanning = Arc::new(AtomicBool::new(false));
        let (analysis_tx, analysis_rx) = crossbeam_channel::bounded::<Track>(4);
        let (done_tx, done_rx) = crossbeam_channel::unbounded();
        let analysis_flag = flag.clone();
        thread::Builder::new()
            .name("dj-analysis".into())
            .spawn(move || {
                while let Ok(track) = analysis_rx.recv() {
                    let result = analysis::analyze_control(
                        Path::new(&track.path),
                        &analysis_flag,
                        Some(&analysis_paused),
                    );
                    let _ = done_tx.send((track.id, result));
                }
            })?;
        thread::Builder::new()
            .name("dj-library".into())
            .spawn(move || {
                loop {
                    while let Ok((id, result)) = done_rx.try_recv() {
                        match result.and_then(|a| {
                            let mut track = get(&conn, id)?;
                            track.duration = a.seconds;
                            if track.bpm.is_none() {
                                track.bpm = a.bpm;
                            }
                            if track.key.is_none() {
                                track.key = a.key.clone();
                            }
                            track.analysis = Some(a);
                            save(&conn, &track)?;
                            etx.send(Event::Analyzed(Box::new(track)))?;
                            Ok(())
                        }) {
                            Ok(()) => {}
                            Err(e) => {
                                let _ = etx.send(Event::Error(e.to_string()));
                            }
                        }
                    }
                    let request = match rx.recv_timeout(std::time::Duration::from_millis(10)) {
                        Ok(r) => r,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    let result: Result<()> = match request {
                        Request::Shutdown => break,
                        Request::Cancel => {
                            flag.store(true, Ordering::Relaxed);
                            Ok(())
                        }
                        Request::Scan(root) => {
                            if scanning.swap(true, Ordering::AcqRel) {
                                Err(anyhow::anyhow!("Skan jest już uruchomiony"))
                            } else {
                                flag.store(false, Ordering::Release);
                                let db_path = path.clone();
                                let cancel = flag.clone();
                                let events = etx.clone();
                                let busy = scanning.clone();
                                thread::Builder::new()
                                    .name("dj-scan".into())
                                    .spawn(move || {
                                        if let Err(e) = open_db(&db_path)
                                            .and_then(|db| scan(&db, &root, &cancel, &events))
                                        {
                                            let _ = events.send(Event::Error(e.to_string()));
                                        }
                                        busy.store(false, Ordering::Release);
                                    })
                                    .map(|_| ())
                                    .map_err(Into::into)
                            }
                        }
                        Request::Query(id, q) => query(&conn, &q).and_then(|rows| {
                            etx.send(Event::Rows(id, rows))?;
                            Ok(())
                        }),
                        Request::Playlists => list_playlists(&conn).and_then(|rows|{etx.send(Event::Playlists(rows))?;Ok(())}),
                        Request::CreatePlaylist(name) => {
                            if name.trim().is_empty()||name.len()>100{Err(anyhow::anyhow!("Nazwa playlisty musi mieć 1–100 znaków"))}else{conn.execute("INSERT INTO playlists(name) VALUES(?)",[name.trim()]).map(|_|()).map_err(Into::into).and_then(|()|{etx.send(Event::Playlists(list_playlists(&conn)?))?;Ok(())})}
                        },
                        Request::DeletePlaylist(id) => conn.execute("DELETE FROM playlists WHERE id=?",[id]).map(|_|()).map_err(Into::into).and_then(|()|{etx.send(Event::Playlists(list_playlists(&conn)?))?;Ok(())}),
                        Request::PlaylistAdd(playlist,track) => conn.execute("INSERT OR IGNORE INTO playlist_tracks(playlist_id,track_id,position) VALUES(?1,?2,COALESCE((SELECT MAX(position)+1 FROM playlist_tracks WHERE playlist_id=?1),0))",params![playlist,track]).map(|_|()).map_err(Into::into).and_then(|()|{etx.send(Event::Saved)?;Ok(())}),
                        Request::PlaylistRemove(playlist,track) => conn.execute("DELETE FROM playlist_tracks WHERE playlist_id=?1 AND track_id=?2",params![playlist,track]).map(|_|()).map_err(Into::into).and_then(|()|{etx.send(Event::Saved)?;Ok(())}),
                        Request::PlaylistMove(playlist,track,direction) => move_playlist(&conn,playlist,track,direction).and_then(|()|{etx.send(Event::Saved)?;Ok(())}),
                        Request::Details(path) => conn.query_row("SELECT id FROM tracks WHERE path=?",[path],|r|r.get::<_,i64>(0)).map_err(anyhow::Error::from).and_then(|id|get(&conn,id)).and_then(|track| {
                            etx.send(Event::Detailed(Box::new(track)))?;
                            Ok(())
                        }),
                        Request::Analyze(id) => get(&conn, id).and_then(|track| {
                            flag.store(false, Ordering::Release);
                            analysis_tx.try_send(track).map_err(|_| {
                                anyhow::anyhow!(
                                    "Kolejka analizy jest pełna; poczekaj na bieżące utwory"
                                )
                            })
                        }),
                        Request::Save(track) => save(&conn, &track).and_then(|()| {
                            etx.send(Event::Saved)?;
                            Ok(())
                        }),
                        Request::Played(id,path) => conn
                            .execute("UPDATE tracks SET played=1 WHERE id=?1 AND path=?2", params![id,path])
                            .map(|_| ())
                            .map_err(Into::into),
                    };
                    if let Err(e) = result {
                        let _ = etx.send(Event::Error(e.to_string()));
                    }
                }
            })?;
        Ok(Self {
            requests: tx,
            events: erx,
            cancel,
            paused,
        })
    }
    pub fn pause_analysis(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.requests.send(Request::Shutdown);
    }
}

pub fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    anyhow::ensure!(
        version <= 1,
        "Biblioteka pochodzi z nowszej wersji aplikacji; zachowano plik"
    );
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS tracks(id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE, title TEXT NOT NULL, artist TEXT NOT NULL DEFAULT '', duration REAL NOT NULL DEFAULT 0, sample_rate INTEGER NOT NULL DEFAULT 0, size INTEGER NOT NULL DEFAULT 0, modified INTEGER NOT NULL DEFAULT 0, bpm REAL, musical_key TEXT, energy INTEGER NOT NULL DEFAULT 3, tags TEXT NOT NULL DEFAULT '', missing INTEGER NOT NULL DEFAULT 0, error TEXT, analysis TEXT, cue TEXT NOT NULL DEFAULT '[0,0,0,0]', downbeat REAL NOT NULL DEFAULT 0, played INTEGER NOT NULL DEFAULT 0); CREATE INDEX IF NOT EXISTS tracks_title ON tracks(title COLLATE NOCASE); CREATE INDEX IF NOT EXISTS tracks_bpm ON tracks(bpm); CREATE INDEX IF NOT EXISTS tracks_key ON tracks(musical_key); PRAGMA user_version=1;")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS track_annotations(track_id INTEGER PRIMARY KEY REFERENCES tracks(id), payload TEXT NOT NULL);")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS playlists(id INTEGER PRIMARY KEY,name TEXT NOT NULL UNIQUE);CREATE TABLE IF NOT EXISTS playlist_tracks(playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,track_id INTEGER NOT NULL REFERENCES tracks(id),position INTEGER NOT NULL,PRIMARY KEY(playlist_id,track_id));CREATE INDEX IF NOT EXISTS playlist_order ON playlist_tracks(playlist_id,position);")?;
    let has_search: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='track_search')",
        [],
        |r| r.get(0),
    )?;
    if !has_search {
        conn.execute_batch("BEGIN IMMEDIATE; CREATE VIRTUAL TABLE track_search USING fts5(title,artist,tags,tokenize='unicode61 remove_diacritics 2'); INSERT INTO track_search(rowid,title,artist,tags) SELECT id,title,artist,tags FROM tracks; CREATE TRIGGER tracks_search_insert AFTER INSERT ON tracks BEGIN INSERT INTO track_search(rowid,title,artist,tags) VALUES(new.id,new.title,new.artist,new.tags); END; CREATE TRIGGER tracks_search_update AFTER UPDATE OF title,artist,tags ON tracks BEGIN DELETE FROM track_search WHERE rowid=old.id; INSERT INTO track_search(rowid,title,artist,tags) VALUES(new.id,new.title,new.artist,new.tags); END; COMMIT;")?;
    }
    Ok(conn)
}
fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    let analysis: Option<String> = row.get(14)?;
    let cue: String = row.get(15)?;
    Ok(Track {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        duration: row.get(4)?,
        sample_rate: row.get(5)?,
        bpm: row.get(8)?,
        key: row.get(9)?,
        energy: row.get(10)?,
        tags: row.get(11)?,
        missing: row.get(12)?,
        error: row.get(13)?,
        analysis: analysis.and_then(|s| serde_json::from_str(&s).ok()),
        cue: serde_json::from_str(&cue).unwrap_or([0.0; 4]),
        downbeat: row.get(16)?,
        played: row.get(17)?,
        markers: Vec::new(),
        phrase_start: 0.0,
        phrase_bars: 16,
    })
}
pub fn query(conn: &Connection, q: &Query) -> Result<Vec<Track>> {
    let terms: Vec<_> = q
        .text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| format!("\"{s}\"*"))
        .collect();
    let fts = terms.join(" AND ");
    let mut s=conn.prepare("SELECT id,path,title,artist,duration,sample_rate,size,modified,bpm,musical_key,energy,tags,missing,error,NULL AS analysis,cue,downbeat,played FROM tracks WHERE ((?7<>'' AND id IN(SELECT rowid FROM track_search WHERE track_search MATCH ?7)) OR (?7='' AND (title LIKE ?1 ESCAPE '!' OR artist LIKE ?1 ESCAPE '!'))) AND (?2 IS NULL OR bpm>=?2) AND (?3 IS NULL OR bpm<=?3) AND (?4 IS NULL OR musical_key=?4) AND (?5=0 OR played=0) AND tags LIKE ?6 ESCAPE '!' AND (?8 IS NULL OR id IN(SELECT track_id FROM playlist_tracks WHERE playlist_id=?8)) ORDER BY CASE WHEN ?8 IS NOT NULL THEN (SELECT position FROM playlist_tracks WHERE playlist_id=?8 AND track_id=tracks.id) ELSE 0 END,title COLLATE NOCASE,id")?;
    let escape = |s: &str| s.replace('!', "!!").replace('%', "!%").replace('_', "!_");
    let rows = s
        .query_map(
            params![
                format!("%{}%", escape(&q.text)),
                q.bpm_min,
                q.bpm_max,
                q.key,
                q.unplayed,
                format!("%{}%", escape(&q.tag)),
                fts,
                q.playlist
            ],
            row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
fn get(conn: &Connection, id: i64) -> Result<Track> {
    let mut track = conn.query_row("SELECT * FROM tracks WHERE id=?", [id], row)?;
    let annotations: Option<String> = conn
        .query_row(
            "SELECT payload FROM track_annotations WHERE track_id=?",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(json) = annotations {
        let data: (Vec<(String, f64)>, f64, usize) = serde_json::from_str(&json)?;
        track.markers = data.0;
        track.phrase_start = data.1;
        track.phrase_bars = data.2;
    }
    Ok(track)
}
pub fn save(conn: &Connection, t: &Track) -> Result<()> {
    anyhow::ensure!(
        t.energy <= 5
            && t.downbeat.is_finite()
            && t.downbeat >= 0.0
            && t.cue.iter().all(|x| x.is_finite() && *x >= 0.0),
        "Nieprawidłowe cue lub parametry utworu"
    );
    if let Some(bpm) = t.bpm {
        anyhow::ensure!(
            bpm.is_finite() && (30.0..=300.0).contains(&bpm),
            "Nieprawidłowe BPM"
        );
    }
    anyhow::ensure!(
        [8, 16, 32].contains(&t.phrase_bars)
            && t.phrase_start.is_finite()
            && t.phrase_start >= 0.0
            && t.markers.len() <= 32
            && t.markers
                .iter()
                .all(|(n, s)| n.len() <= 64 && s.is_finite() && *s >= 0.0),
        "Nieprawidłowe markery fraz"
    );
    let analysis = t.analysis.as_ref().map(serde_json::to_string).transpose()?;
    let annotations = serde_json::to_string(&(&t.markers, t.phrase_start, t.phrase_bars))?;
    conn.execute_batch("SAVEPOINT save_track")?;
    let result = (|| {
        let changed=conn.execute("UPDATE tracks SET bpm=?1,musical_key=?2,energy=?3,tags=?4,analysis=COALESCE(?5,analysis),cue=?6,downbeat=?7,duration=?8 WHERE id=?9 AND path=?10",params![t.bpm,t.key,t.energy,t.tags,analysis,serde_json::to_string(&t.cue)?,t.downbeat,t.duration,t.id,t.path])?;
        anyhow::ensure!(
            changed == 1,
            "Utwór z sesji nie jest w tym indeksie; wskaż jego katalog"
        );
        conn.execute("INSERT INTO track_annotations(track_id,payload) VALUES(?1,?2) ON CONFLICT(track_id) DO UPDATE SET payload=excluded.payload",params![t.id,annotations])?;
        Ok::<_, anyhow::Error>(())
    })();
    if let Err(e) = result {
        conn.execute_batch("ROLLBACK TO save_track; RELEASE save_track")?;
        return Err(e);
    }
    conn.execute_batch("RELEASE save_track")?;
    Ok(())
}
pub fn list_playlists(conn: &Connection) -> Result<Vec<(i64, String)>> {
    Ok(conn
        .prepare("SELECT id,name FROM playlists ORDER BY name COLLATE NOCASE")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}
fn move_playlist(conn: &Connection, playlist: i64, track: i64, direction: i32) -> Result<()> {
    let rows = conn
        .prepare(
            "SELECT track_id,position FROM playlist_tracks WHERE playlist_id=? ORDER BY position",
        )?
        .query_map([playlist], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if let Some(i) = rows.iter().position(|(id, _)| *id == track) {
        let next = (i as i32 + direction.signum()).clamp(0, rows.len() as i32 - 1) as usize;
        if next != i {
            conn.execute_batch("SAVEPOINT reorder")?;
            let result = (|| {
                conn.execute(
                    "UPDATE playlist_tracks SET position=?1 WHERE playlist_id=?2 AND track_id=?3",
                    params![rows[next].1, playlist, rows[i].0],
                )?;
                conn.execute(
                    "UPDATE playlist_tracks SET position=?1 WHERE playlist_id=?2 AND track_id=?3",
                    params![rows[i].1, playlist, rows[next].0],
                )?;
                Ok::<_, rusqlite::Error>(())
            })();
            if let Err(e) = result {
                conn.execute_batch("ROLLBACK TO reorder; RELEASE reorder")?;
                return Err(e.into());
            }
            conn.execute_batch("RELEASE reorder")?;
        }
    }
    Ok(())
}

pub fn scan(
    conn: &Connection,
    root: &Path,
    cancel: &AtomicBool,
    events: &Sender<Event>,
) -> Result<()> {
    anyhow::ensure!(root.is_dir(), "Wskaż dostępny katalog muzyczny");
    let mut count = 0;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let _ = events.send(Event::Error(e.to_string()));
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !["mp3", "wav", "flac", "aif", "aiff", "ogg", "m4a"].contains(&ext.as_str()) {
            continue;
        }
        let meta = entry.metadata()?;
        let modified = meta
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let full = path.canonicalize()?.to_string_lossy().into_owned();
        let existing = conn
            .query_row(
                "SELECT size,modified FROM tracks WHERE path=?",
                [&full],
                |r| Ok((r.get::<_, u64>(0)?, r.get::<_, i64>(1)?)),
            )
            .ok();
        if existing != Some((meta.len(), modified)) {
            let mut title = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("Utwór")
                .to_owned();
            let mut artist = String::new();
            let mut duration = 0.0;
            let mut sr = 0;
            let mut error = None;
            match Decoder::open(path) {
                Ok(mut d) => {
                    duration = d.duration;
                    sr = d.sample_rate;
                    let (t, a) = d.tags();
                    if !t.is_empty() {
                        title = t;
                    }
                    artist = a;
                }
                Err(e) => error = Some(e.to_string()),
            }
            conn.execute("INSERT INTO tracks(path,title,artist,duration,sample_rate,size,modified,error) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(path) DO UPDATE SET title=excluded.title,artist=excluded.artist,duration=excluded.duration,sample_rate=excluded.sample_rate,size=excluded.size,modified=excluded.modified,error=excluded.error,missing=0,analysis=NULL",params![full,title,artist,duration,sr,meta.len(),modified,error])?;
        } else {
            conn.execute("UPDATE tracks SET missing=0 WHERE path=?", [&full])?;
        }
        count += 1;
        if count % 50 == 0 {
            let _ = events.send(Event::Progress(count));
        }
    }
    // Availability is checked without deleting rows or annotations.
    let paths = conn
        .prepare("SELECT id,path FROM tracks")?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, path) in paths {
        conn.execute(
            "UPDATE tracks SET missing=?1 WHERE id=?2",
            params![!Path::new(&path).is_file(), id],
        )?;
    }
    events
        .send(Event::Scanned(count))
        .context("Biblioteka zamknięta")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playlist_order_and_unicode_search_are_persistent() {
        let conn = open_db(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO tracks(path,title)VALUES('one.wav','Żółty rytm'),('two.wav','Inny utwór')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO playlists(name)VALUES('Set techno')", [])
            .unwrap();
        conn.execute_batch("INSERT INTO playlist_tracks VALUES(1,1,0),(1,2,1)")
            .unwrap();
        let q = Query {
            playlist: Some(1),
            ..Query::default()
        };
        assert_eq!(query(&conn, &q).unwrap()[0].title, "Żółty rytm");
        move_playlist(&conn, 1, 2, -1).unwrap();
        assert_eq!(query(&conn, &q).unwrap()[0].title, "Inny utwór");
        let q = Query {
            text: "żół".into(),
            ..Query::default()
        };
        assert_eq!(query(&conn, &q).unwrap().len(), 1);
    }
    #[test]
    fn indexed_queries_cover_large_collection_and_literal_search() {
        let conn = open_db(Path::new(":memory:")).unwrap();
        conn.execute_batch("BEGIN").unwrap();
        {
            let mut insert = conn
                .prepare("INSERT INTO tracks(path,title,bpm,musical_key) VALUES(?1,?2,?3,'Am')")
                .unwrap();
            for i in 0..50_000 {
                insert
                    .execute(params![
                        format!("/synthetic/{i}.wav"),
                        format!("Techno {i:05}"),
                        136.0 + (i % 8) as f32
                    ])
                    .unwrap();
            }
        }
        conn.execute_batch("COMMIT").unwrap();
        let q = Query {
            text: "Techno 123".into(),
            bpm_min: Some(136.0),
            bpm_max: Some(144.0),
            ..Query::default()
        };
        let mut timings = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            let rows = query(&conn, &q).unwrap();
            assert_eq!(rows.len(), 100);
            timings.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        timings.sort_by(f64::total_cmp);
        eprintln!("50k synthetic metadata, query p95 {:.2} ms", timings[18]);
        assert!(timings[18] < 100.0);
        assert!(
            query(
                &conn,
                &Query {
                    text: "%".into(),
                    ..Query::default()
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    fn wav(path: &Path) {
        let mut w = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..4410 {
            w.write_sample(if i % 2 == 0 { 1000_i16 } else { 0 })
                .unwrap();
            w.write_sample(0_i16).unwrap();
        }
        w.finalize().unwrap();
    }
    #[test]
    fn incremental_scan_preserves_annotations_and_missing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Żółty test.wav");
        wav(&file);
        let conn = open_db(&dir.path().join("library.sqlite3")).unwrap();
        let (tx, _rx) = crossbeam_channel::unbounded();
        let cancel = AtomicBool::new(false);
        scan(&conn, dir.path(), &cancel, &tx).unwrap();
        let mut t = query(&conn, &Query::default()).unwrap().remove(0);
        t.bpm = Some(140.0);
        t.cue[1] = 0.05;
        save(&conn, &t).unwrap();
        scan(&conn, dir.path(), &cancel, &tx).unwrap();
        let rows = query(&conn, &Query::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].bpm, Some(140.0));
        assert_eq!(rows[0].cue[1], 0.05);
        std::fs::rename(&file, dir.path().join("moved.txt")).unwrap();
        scan(&conn, dir.path(), &cancel, &tx).unwrap();
        assert!(query(&conn, &Query::default()).unwrap()[0].missing);
    }
    #[test]
    fn decoder_preserves_stereo_and_seeks() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("stereo.wav");
        wav(&p);
        let mut d = Decoder::open(&p).unwrap();
        for _ in 0..100 {
            assert_eq!(d.next_frame().unwrap().unwrap()[1], 0.0);
        }
        d.seek(0.05).unwrap();
        assert!(d.next_frame().unwrap().is_some());
    }
    #[test]
    fn corrupted_file_does_not_abort_scan() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.mp3"), b"garbage").unwrap();
        wav(&dir.path().join("valid.wav"));
        let conn = open_db(&dir.path().join("db")).unwrap();
        let (tx, _rx) = crossbeam_channel::unbounded();
        scan(&conn, dir.path(), &AtomicBool::new(false), &tx).unwrap();
        let rows = query(&conn, &Query::default()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().filter(|t| t.error.is_some()).count(), 1);
    }
}
