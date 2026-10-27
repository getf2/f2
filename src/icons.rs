//! File icons as Finder shows them: the system icon for a file's type, a
//! folder's or app's own icon, and Quick Look thumbnails where a file has
//! one. Type icons are made on the spot (one per extension); per-file icons
//! and thumbnails come from worker threads (one pool for every window),
//! newest request first, and show up as they arrive.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui::RenderImage;

use crate::listing::Entry;
use crate::macos;

const WORKERS: usize = 4;
/// Per-file images kept in memory (thumbnails, folder and app icons).
const MAX_BYTES: usize = 256 << 20;
/// Extensions whose files never had a thumbnail stop asking after this many.
const GIVE_UP_AFTER: u32 = 3;
/// Requests waiting; past this the oldest (scrolled away by now) are dropped.
const MAX_QUEUED: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum What {
    Icon,
    Thumbnail,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Key {
    path: PathBuf,
    what: What,
    px: u32,
    /// For thumbnails, so an edited file gets a new one.
    mtime: i64,
    /// A tagged folder's color (0xRRGGBB, as Finder 26 tints them), or 0.
    tint: u32,
}

pub struct Done {
    key: Key,
    image: Option<RenderImage>,
}

enum Slot {
    Pending,
    Ready(Arc<RenderImage>),
    Missing,
}

/// A request, and where its answer goes.
struct Job {
    key: Key,
    reply: UnboundedSender<Done>,
}

#[derive(Default)]
struct Queue {
    jobs: Mutex<Vec<Job>>,
    wake: Condvar,
}

/// The pool's queue, its workers started on first use.
fn queue() -> &'static Queue {
    static QUEUE: OnceLock<Queue> = OnceLock::new();
    QUEUE.get_or_init(|| {
        for i in 0..WORKERS {
            std::thread::Builder::new().name(format!("icons-{i}")).spawn(work).unwrap();
        }
        Queue::default()
    })
}

/// An image to show for a file, and whether it's a preview of its contents
/// (rather than an icon).
pub struct Shown {
    pub image: Arc<RenderImage>,
    pub thumbnail: bool,
}

pub struct Icons {
    types: HashMap<(String, bool, u32), Option<Arc<RenderImage>>>,
    files: HashMap<Key, Slot>,
    /// Ready per-file images, oldest first, for eviction.
    order: VecDeque<(Key, usize)>,
    bytes: usize,
    /// Thumbnail misses per extension; `None` once one of its files had one.
    misses: HashMap<String, Option<u32>>,
    reply: UnboundedSender<Done>,
    /// What per-file images may take, at most.
    max: usize,
}

impl Icons {
    /// The cache and the stream of finished requests to feed back into
    /// [`Icons::finish`].
    pub fn new() -> (Icons, UnboundedReceiver<Done>) {
        Icons::with_limit(MAX_BYTES)
    }

    /// The same, keeping `max` bytes of per-file images at most.
    pub fn with_limit(max: usize) -> (Icons, UnboundedReceiver<Done>) {
        let (reply, rx) = unbounded();
        let icons = Icons {
            types: HashMap::new(),
            files: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            misses: HashMap::new(),
            reply,
            max,
        };
        (icons, rx)
    }

    /// The best image there is for `entry` right now at `px` pixels: its
    /// thumbnail, else its own icon (folders, apps), else its type's icon.
    /// Asks for whatever better one is missing.
    pub fn get(&mut self, entry: &Entry, px: u32) -> Option<Shown> {
        let ext = entry.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        // (A server's item isn't here to look at: its type's icon.)
        let here = !crate::remote::is_remote(&entry.path);
        // An alias shows its own icon (the original's, with an arrow).
        let wants_thumbnail = here
            && !entry.is_dir
            && !entry.alias
            && entry.size > 0
            && self.misses.get(&ext).copied().flatten().is_none_or(|n| n < GIVE_UP_AFTER);
        if wants_thumbnail && let Some(image) = self.file(&entry.path, What::Thumbnail, px, entry.mtime) {
            return Some(Shown { image, thumbnail: true });
        }
        // (A tagged folder in its tag's color, as Finder 26 shows them;
        // unless that's turned off in Finder's Settings.)
        let tint = if entry.is_dir && !entry.is_bundle() && !entry.alias && tinting() {
            entry.tags.iter().find_map(|(_, color)| crate::style::tag_rgb(*color)).unwrap_or(0)
        } else {
            0
        };
        if here && (entry.is_dir || entry.alias) && let Some(image) = self.file_tinted(&entry.path, What::Icon, px, 0, tint) {
            return Some(Shown { image, thumbnail: false });
        }
        let image = self
            .types
            .entry((ext, entry.is_dir && !entry.is_bundle(), px))
            .or_insert_with_key(|(ext, is_dir, px)| macos::type_icon(ext, *is_dir, *px).map(Arc::new))
            .clone()?;
        Some(Shown { image, thumbnail: false })
    }

    fn file(&mut self, path: &Path, what: What, px: u32, mtime: i64) -> Option<Arc<RenderImage>> {
        self.file_tinted(path, what, px, mtime, 0)
    }

    fn file_tinted(&mut self, path: &Path, what: What, px: u32, mtime: i64, tint: u32) -> Option<Arc<RenderImage>> {
        let key = Key { path: path.to_path_buf(), what, px, mtime, tint };
        match self.files.get(&key) {
            Some(Slot::Ready(image)) => Some(image.clone()),
            Some(_) => None,
            None => {
                self.files.insert(key.clone(), Slot::Pending);
                let queue = queue();
                let mut jobs = queue.jobs.lock().unwrap();
                let mine = jobs.iter().filter(|j| j.reply.same_receiver(&self.reply)).count();
                if mine >= MAX_QUEUED
                    && let Some(oldest) = jobs.iter().position(|j| j.reply.same_receiver(&self.reply))
                {
                    self.files.remove(&jobs.remove(oldest).key);
                }
                jobs.push(Job { key, reply: self.reply.clone() });
                queue.wake.notify_one();
                None
            }
        }
    }

    /// Store a finished request. Returns the images evicted to stay under
    /// budget, for the caller to drop from the GPU (`App::drop_image`).
    pub fn finish(&mut self, done: Done) -> Vec<Arc<RenderImage>> {
        let Done { key, image } = done;
        if key.what == What::Thumbnail {
            let ext = key.path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let misses = self.misses.entry(ext).or_insert(Some(0));
            *misses = if image.is_some() { None } else { misses.map(|n| n + 1) };
        }
        let Some(image) = image else {
            self.files.insert(key, Slot::Missing);
            return Vec::new();
        };
        let size = image.as_bytes(0).map_or(0, <[u8]>::len);
        self.files.insert(key.clone(), Slot::Ready(Arc::new(image)));
        self.order.push_back((key, size));
        self.bytes += size;

        let mut evicted = Vec::new();
        while self.bytes > self.max
            && let Some((key, size)) = self.order.pop_front()
        {
            self.bytes -= size;
            if let Some(Slot::Ready(image)) = self.files.remove(&key) {
                evicted.push(image);
            }
        }
        evicted
    }

    /// Look this item's images up again (its icon changed).
    pub fn forget(&mut self, path: &Path) {
        self.files.retain(|key, _| key.path != path);
        self.order.retain(|(key, _)| key.path != path);
    }

    /// Drop requests not started yet (the folder changed): they'll be asked
    /// for again if they're still on screen.
    pub fn forget_queued(&mut self) {
        let mut jobs = queue().jobs.lock().unwrap();
        let (mine, others): (Vec<Job>, Vec<Job>) = jobs.drain(..).partition(|j| j.reply.same_receiver(&self.reply));
        *jobs = others;
        for job in mine {
            self.files.remove(&job.key);
        }
    }
}

/// Whether tagged folders are tinted (Finder's setting: read once a few
/// seconds at most, as icons are asked for all the time).
fn tinting() -> bool {
    static KNOWN: Mutex<Option<(std::time::Instant, bool)>> = Mutex::new(None);
    let mut known = KNOWN.lock().unwrap();
    match *known {
        Some((at, on)) if at.elapsed() < std::time::Duration::from_secs(5) => on,
        _ => {
            let on = macos::tag_tinting();
            *known = Some((std::time::Instant::now(), on));
            on
        }
    }
}

fn work() {
    unsafe { libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0) };
    let queue = queue();
    loop {
        let Job { key, reply } = {
            let mut jobs = queue.jobs.lock().unwrap();
            loop {
                match jobs.pop() {
                    Some(job) => break job,
                    None => jobs = queue.wake.wait(jobs).unwrap(),
                }
            }
        };
        // (Its window may be gone; nobody's waiting then.)
        if reply.is_closed() {
            continue;
        }
        let image = match key.what {
            What::Icon if key.tint != 0 => macos::folder_icon_tinted(&key.path, key.px, key.tint),
            What::Icon => macos::file_icon(&key.path, key.px),
            What::Thumbnail => macos::thumbnail(&key.path, key.px),
        };
        let _ = reply.unbounded_send(Done { key, image });
    }
}
