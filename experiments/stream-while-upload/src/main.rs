//! Prototype: can FFmpeg decode a file that is still being uploaded,
//! if the input layer *waits* for missing bytes instead of returning EOF?
//!
//! Experiments per sample file:
//!   1. baseline  - decode the complete file via `format::input(path)`.
//!   2. growing   - an uploader thread copies the file chunk by chunk into a
//!                  partial file; the decoder reads the partial file through
//!                  `StreamIo` (custom AVIO) whose `read` blocks on a Condvar
//!                  until the requested bytes have arrived, returning EOF only
//!                  after the upload finished.
//!   3. truncated - control: open a static 40%-truncated copy with plain
//!                  `format::input` and see what happens without a waiting input.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ffmpeg_next as ffmpeg;
use ffmpeg::format::context::StreamIo;

const CHUNK: usize = 64 * 1024;
const CHUNK_INTERVAL: Duration = Duration::from_millis(40);
const DECODE_TIMEOUT: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// Shared upload state
// ---------------------------------------------------------------------------

struct GrowState {
    /// Contiguous bytes already present in the partial file.
    available: u64,
    /// Total size of the complete file (known upfront, like an upload API
    /// that declares `total_size` at begin).
    total: u64,
    /// Uploader is done (successfully or not); only now may reads return EOF.
    finished: bool,
    started: Instant,
    /// (available bytes, elapsed) recorded when the first frame was decoded.
    first_frame: Option<(u64, Duration)>,
}

type Shared = Arc<(Mutex<GrowState>, Condvar)>;

fn new_shared(total: u64) -> Shared {
    Arc::new((
        Mutex::new(GrowState {
            available: 0,
            total,
            finished: false,
            started: Instant::now(),
            first_frame: None,
        }),
        Condvar::new(),
    ))
}

// ---------------------------------------------------------------------------
// GrowingFile: a Read+Seek view over the partial file that waits for bytes.
// ---------------------------------------------------------------------------

struct GrowingFile {
    file: File,
    pos: u64,
    total: u64,
    shared: Shared,
}

impl GrowingFile {
    /// Block until at least `need` bytes are available, or the upload finished.
    fn wait_until(&self, need: u64) {
        let (lock, cvar) = &*self.shared;
        let mut st = lock.lock().unwrap();
        while st.available < need && !st.finished {
            st = cvar.wait(st).unwrap();
        }
    }
}

impl Read for GrowingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Wait for at least one byte at the current position. Only when the
        // upload is finished and nothing is left do we report EOF (Ok(0),
        // which StreamIo translates to AVERROR_EOF).
        self.wait_until(self.pos + 1);
        let avail = {
            let (lock, _) = &*self.shared;
            lock.lock().unwrap().available
        };
        if self.pos >= avail {
            return Ok(0);
        }
        let want = ((buf.len() as u64).min(avail - self.pos)) as usize;
        let mut got = 0usize;
        while got < want {
            let n = self.file.read_at(&mut buf[got..want], self.pos + got as u64)?;
            if n == 0 {
                break;
            }
            got += n;
        }
        self.pos += got as u64;
        Ok(got)
    }
}

impl Seek for GrowingFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        // Positions are logical, against the *final* total size: SeekFrom::End
        // therefore reports the complete file size even mid-upload (this is
        // what AVSEEK_SIZE probes). Seeking past `available` is allowed; the
        // following read will wait for those bytes.
        let target: i128 = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(off) => self.pos as i128 + off as i128,
            SeekFrom::End(off) => self.total as i128 + off as i128,
        };
        if target < 0 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        self.pos = target as u64;
        Ok(self.pos)
    }
}

// ---------------------------------------------------------------------------
// Uploader thread: copies src -> partial file chunk by chunk.
// ---------------------------------------------------------------------------

fn spawn_uploader(src: &Path, dst: &Path, shared: Shared) -> thread::JoinHandle<()> {
    let src = src.to_path_buf();
    let dst = dst.to_path_buf();
    thread::spawn(move || {
        let data = std::fs::read(&src).expect("read source");
        let mut out = File::create(&dst).expect("create partial");
        let (lock, cvar) = &*shared;
        let mut off = 0usize;
        while off < data.len() {
            let n = CHUNK.min(data.len() - off);
            out.write_all(&data[off..off + n]).expect("write chunk");
            out.flush().expect("flush chunk");
            off += n;
            {
                let mut st = lock.lock().unwrap();
                st.available = off as u64;
            }
            cvar.notify_all();
            thread::sleep(CHUNK_INTERVAL);
        }
        {
            let mut st = lock.lock().unwrap();
            st.finished = true;
        }
        cvar.notify_all();
    })
}

// ---------------------------------------------------------------------------
// Decode helpers
// ---------------------------------------------------------------------------

/// Decode the best video stream, counting frames into `count` (which keeps
/// its value even if the stream then errors). `on_first_frame` fires once,
/// at the moment the first frame comes out of the decoder.
fn decode_count(
    input: &mut ffmpeg::format::context::Input,
    count: &mut usize,
    on_first_frame: &mut dyn FnMut(),
) -> Result<(), ffmpeg::Error> {
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    let stream_index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
        .decoder()
        .video()?;

    let mut frame = ffmpeg::frame::Video::empty();
    for (s, packet) in input.packets() {
        if s.index() != stream_index {
            continue;
        }
        decoder.send_packet(&packet)?;
        while decoder.receive_frame(&mut frame).is_ok() {
            if *count == 0 {
                on_first_frame();
            }
            *count += 1;
        }
    }
    decoder.send_eof()?;
    while decoder.receive_frame(&mut frame).is_ok() {
        if *count == 0 {
            on_first_frame();
        }
        *count += 1;
    }
    Ok(())
}

fn baseline(path: &Path) -> Result<(usize, Duration), ffmpeg::Error> {
    let t0 = Instant::now();
    let mut input = ffmpeg::format::input(path)?;
    let mut count = 0usize;
    decode_count(&mut input, &mut count, &mut || {})?;
    Ok((count, t0.elapsed()))
}

// ---------------------------------------------------------------------------
// Growing experiment
// ---------------------------------------------------------------------------

struct GrowingResult {
    frames: Result<usize, String>,
    first_frame: Option<(u64, Duration)>,
    total: u64,
    elapsed: Duration,
}

fn growing(src: &Path, partial: &Path, filename_hint: &str) -> GrowingResult {
    let total = std::fs::metadata(src).unwrap().len();
    // Pre-create the partial file so the decoder thread can open it before
    // the uploader thread writes the first chunk.
    let _ = std::fs::remove_file(partial);
    File::create(partial).unwrap();
    let shared = new_shared(total);
    let _uploader = spawn_uploader(src, partial, Arc::clone(&shared));

    let shared_dec = Arc::clone(&shared);
    let partial_dec = partial.to_path_buf();
    let hint = filename_hint.to_string();
    let (tx, rx) = mpsc::channel();
    let t0 = Instant::now();
    thread::spawn(move || {
        let result = (|| -> Result<usize, String> {
            let file = File::open(&partial_dec).map_err(|e| format!("open partial: {e}"))?;
            let growing = GrowingFile {
                file,
                pos: 0,
                total,
                shared: Arc::clone(&shared_dec),
            };
            let io = StreamIo::from_read_seek(growing).map_err(|e| format!("StreamIo: {e:?}"))?;
            let mut input = ffmpeg::format::input_from_stream(io, Some(&hint), None)
                .map_err(|e| format!("input_from_stream: {e:?}"))?;
            let shared_ff = Arc::clone(&shared_dec);
            let mut on_first = move || {
                let (lock, _) = &*shared_ff;
                let mut st = lock.lock().unwrap();
                if st.first_frame.is_none() {
                    st.first_frame = Some((st.available, st.started.elapsed()));
                }
            };
            let mut count = 0usize;
            match decode_count(&mut input, &mut count, &mut on_first) {
                Ok(()) => Ok(count),
                Err(e) => Err(format!("decode stopped after {count} frames: {e:?}")),
            }
        })();
        let _ = tx.send(result);
    });

    let frames = match rx.recv_timeout(DECODE_TIMEOUT) {
        Ok(r) => r,
        Err(_) => Err(format!("timeout after {DECODE_TIMEOUT:?}")),
    };
    let st = shared.0.lock().unwrap();
    GrowingResult {
        frames,
        first_frame: st.first_frame,
        total,
        elapsed: t0.elapsed(),
    }
}

// ---------------------------------------------------------------------------
// Truncated control experiment
// ---------------------------------------------------------------------------

fn truncated(src: &Path, pct: u64, work: &Path) -> String {
    let data = std::fs::read(src).unwrap();
    let cut = (data.len() as u64 * pct / 100) as usize;
    let name = src.file_name().unwrap().to_string_lossy();
    let path = work.join(format!("trunc{pct}-{name}"));
    std::fs::write(&path, &data[..cut]).unwrap();
    match ffmpeg::format::input(&path) {
        Ok(mut input) => {
            let mut count = 0usize;
            match decode_count(&mut input, &mut count, &mut || {}) {
                Ok(()) => format!("opened; decoded {count} frames; stream then ended"),
                Err(e) => format!("opened; decoded {count} frames; then error: {e:?}"),
            }
        }
        Err(e) => format!("open failed: {e:?}"),
    }
}

// ---------------------------------------------------------------------------

fn main() {
    ffmpeg::init().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let samples = root.join("samples");
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();

    let files = ["faststart.mp4", "tail.mp4", "sample.mkv"];
    println!("chunk={} KiB interval={:?}", CHUNK / 1024, CHUNK_INTERVAL);
    for name in files {
        let src = samples.join(name);
        let total = std::fs::metadata(&src).unwrap().len();
        println!("\n=== {name} ({total} bytes) ===");

        match baseline(&src) {
            Ok((n, d)) => println!("baseline : {n} frames in {d:?}"),
            Err(e) => println!("baseline : ERROR {e:?}"),
        }

        let partial = work.join(format!("{name}.part"));
        let r = growing(&src, &partial, name);
        match (&r.frames, r.first_frame) {
            (Ok(n), Some((avail, when))) => println!(
                "growing  : {n} frames; first frame at {:.1}% uploaded ({avail} / {} bytes), {:?} after start; total {:?}",
                avail as f64 * 100.0 / r.total as f64,
                r.total,
                when,
                r.elapsed
            ),
            (Ok(n), None) => {
                println!("growing  : {n} frames; no first-frame record; total {:?}", r.elapsed)
            }
            (Err(e), ff) => println!("growing  : FAILED: {e} (first frame: {ff:?})"),
        }

        println!("trunc40% : {}", truncated(&src, 40, &work));
    }
}
