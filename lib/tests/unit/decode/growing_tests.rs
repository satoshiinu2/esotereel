use super::*;

use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::decode::videostreamer::VideoStreamer;

fn partial_file(tag: &str, contents: &[u8]) -> (File, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "esotereel-growing-test-{}-{tag}",
        std::process::id()
    ));
    let mut file = File::create(&path).unwrap();
    file.write_all(contents).unwrap();
    (File::open(&path).unwrap(), path)
}

#[test]
fn prefix_advances_only_over_contiguous_ranges() {
    let progress = UploadProgress::new(200);
    assert_eq!(progress.prefix_len(), 0);

    progress.record_range(100, 50);
    assert_eq!(progress.prefix_len(), 0);

    progress.record_range(0, 100);
    assert_eq!(progress.prefix_len(), 150);

    progress.record_range(160, 40);
    assert_eq!(progress.prefix_len(), 150);

    progress.record_range(150, 10);
    assert_eq!(progress.prefix_len(), 200);
}

#[test]
fn record_range_merges_overlaps_and_clips_to_total_size() {
    let progress = UploadProgress::new(100);

    progress.record_range(0, 60);
    progress.record_range(40, 60);
    assert_eq!(progress.prefix_len(), 100);

    progress.record_range(90, 60);
    assert_eq!(progress.prefix_len(), 100);
}

#[test]
fn read_blocks_until_bytes_arrive_then_reads_them() {
    let progress = Arc::new(UploadProgress::new(5));
    let (file, path) = partial_file("blocks", b"hello");
    let mut growing = GrowingFile::new(file, Arc::clone(&progress));

    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut buf = [0u8; 5];
        let result = growing.read(&mut buf).map(|n| (n, buf));
        let _ = tx.send(result);
    });

    // 到着前は EOF でもデータでもなく、ブロックし続けます。
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    progress.record_range(0, 5);
    let (n, buf) = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("reader should wake once bytes arrive")
        .unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf, b"hello");
    reader.join().unwrap();

    let _ = std::fs::remove_file(path);
}

#[test]
fn read_reports_eof_only_once_the_upload_finished() {
    let progress = Arc::new(UploadProgress::new(3));
    let (file, path) = partial_file("eof", b"abc");
    progress.record_range(0, 3);
    let mut growing = GrowingFile::new(file, Arc::clone(&progress));

    let mut buf = [0u8; 3];
    assert_eq!(growing.read(&mut buf).unwrap(), 3);

    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut buf = [0u8; 1];
        let _ = tx.send(growing.read(&mut buf));
    });

    // 完了前は読み尽くしても EOF を返しません。
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    progress.finish();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5))
            .expect("reader should wake once the upload finished")
            .unwrap(),
        0
    );
    reader.join().unwrap();

    let _ = std::fs::remove_file(path);
}

#[test]
fn abort_wakes_a_blocked_reader_with_an_error() {
    let progress = Arc::new(UploadProgress::new(10));
    let (file, path) = partial_file("abort", b"");
    let mut growing = GrowingFile::new(file, Arc::clone(&progress));

    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut buf = [0u8; 4];
        let _ = tx.send(growing.read(&mut buf));
    });

    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());

    progress.abort();
    let error = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("reader should wake once the upload is aborted")
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
    reader.join().unwrap();

    let _ = std::fs::remove_file(path);
}

#[test]
fn seek_end_reports_the_total_size_before_any_bytes_arrive() {
    let progress = Arc::new(UploadProgress::new(1234));
    let (file, path) = partial_file("seek", b"");
    let mut growing = GrowingFile::new(file, progress);

    assert_eq!(growing.seek(SeekFrom::End(0)).unwrap(), 1234);
    assert_eq!(growing.seek(SeekFrom::Start(42)).unwrap(), 42);
    assert!(growing.seek(SeekFrom::End(-2000)).is_err());

    let _ = std::fs::remove_file(path);
}

/// アップロード完了前にデコードを開始し、完了ファイルと同じフレーム数まで
/// 完走できることを確認します。faststart 形式のサンプルを前提とするため、
/// `ESOTEREEL_TEST_VIDEO` に faststart MP4 を指定して明示的に実行します。
#[test]
#[ignore = "requires ESOTEREEL_TEST_VIDEO pointing to a faststart MP4"]
fn decode_starts_before_upload_completes_and_finishes() {
    let _ = ffmpeg_next::init();
    let src =
        std::env::var("ESOTEREEL_TEST_VIDEO").expect("set ESOTEREEL_TEST_VIDEO to a faststart MP4");
    let data = std::fs::read(&src).unwrap();
    let total = data.len() as u64;

    let baseline_frames = {
        let mut streamer = VideoStreamer::new(&src).unwrap();
        let mut count = 0;
        while streamer.next_frame().is_some() {
            count += 1;
        }
        count
    };
    assert!(baseline_frames > 0);

    let partial_path = std::env::temp_dir().join(format!(
        "esotereel-growing-test-{}.part",
        std::process::id()
    ));
    File::create(&partial_path).unwrap();
    let progress = Arc::new(UploadProgress::new(total));

    let uploader = {
        let progress = Arc::clone(&progress);
        let partial_path = partial_path.clone();
        thread::spawn(move || {
            let mut out = File::create(&partial_path).unwrap();
            let mut offset = 0usize;
            while offset < data.len() {
                let n = (64 * 1024).min(data.len() - offset);
                out.write_all(&data[offset..offset + n]).unwrap();
                out.flush().unwrap();
                progress.record_range(offset as u64, n as u64);
                offset += n;
                thread::sleep(Duration::from_millis(30));
            }
            progress.finish();
        })
    };

    let (tx, rx) = mpsc::channel();
    let decode = {
        let progress = Arc::clone(&progress);
        let partial_path = partial_path.clone();
        thread::spawn(move || {
            let result = (|| {
                let mut streamer = VideoStreamer::new_growing(&partial_path, progress.clone())
                    .map_err(|e| format!("open growing stream: {e:?}"))?;
                let mut count = 0;
                let mut first_frame_prefix = None;
                while streamer.next_frame().is_some() {
                    if first_frame_prefix.is_none() {
                        first_frame_prefix = Some(progress.prefix_len());
                    }
                    count += 1;
                }
                Ok::<_, String>((count, first_frame_prefix))
            })();
            let _ = tx.send(result);
        })
    };

    let (frames, first_frame_prefix) = rx
        .recv_timeout(Duration::from_secs(120))
        .expect("growing decode should not hang")
        .unwrap();
    decode.join().unwrap();
    uploader.join().unwrap();

    assert_eq!(frames, baseline_frames);
    let first_frame_prefix = first_frame_prefix.expect("at least one frame decoded");
    assert!(
        first_frame_prefix < total,
        "first frame should arrive before the upload completes: {first_frame_prefix} of {total}"
    );

    let _ = std::fs::remove_file(partial_path);
}
