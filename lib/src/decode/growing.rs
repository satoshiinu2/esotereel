//! アップロード中のファイルを読むための「待てる入力」です。
//!
//! アップロードが完了する前のファイルは、通常の入力で開くと未到着部分で
//! EOF 扱いになり、デコードが途中で止まったり無言で終了したりします。
//! ここでは到着済み範囲を共有する [`UploadProgress`] と、それを参照する
//! [`GrowingFile`]（`Read + Seek`）を提供し、未到着バイトでは EOF を返さず
//! 到着までブロックさせることで、アップロード中のデコード開始を可能にします。
//!
//! 対応範囲と制約:
//! - 読み進められるのは「先頭から連続して到着済みの接頭辞」だけです。
//!   チャンクが順不同に届いても、接頭辞に穴がある間はその先を読みません。
//! - 接頭辞の外へのシーク自体は許可しますが、その後の読みは対象バイトの
//!   到着までブロックします。末尾に索引がある形式（moov が末尾の MP4 等）は、
//!   索引が届くまでデコードを開始できません。
//! - アップロードが中断された場合は待機中の読みがエラーで目覚めます。
//!   完了前に EOF を返すことはありません。

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Range;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// アップロードの進行状態です。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ProgressState {
    /// 受付中。未到着バイトの読みはブロックします。
    Active,
    /// 正常完了。以後は通常ファイルと同じく EOF を返します。
    Finished,
    /// 中断。待機中の読みはエラーになります。
    Aborted,
}

struct ProgressInner {
    /// 受信済みのバイト範囲。到着順によらずマージ済み・開始位置順を保ちます。
    ranges: Vec<Range<u64>>,
    /// 先頭から連続して到着済みのバイト数（接頭辞の長さ）。
    prefix: u64,
    state: ProgressState,
}

/// 1件のアップロードについて、到着済み範囲と完了/中断を共有する状態です。
///
/// 書き込み側（アップロード処理）が [`record_range`](Self::record_range) で
/// 到着を知らせ、読み側（[`GrowingFile`]）が接頭辞の伸びを待機します。
/// `Arc` で共有し、書き込み側と読み側を別スレッドで使えます。
pub struct UploadProgress {
    total_size: u64,
    inner: Mutex<ProgressInner>,
    changed: Condvar,
}

impl UploadProgress {
    /// 宣言された総サイズで初期化します。
    pub fn new(total_size: u64) -> Self {
        Self {
            total_size,
            inner: Mutex::new(ProgressInner {
                ranges: Vec::new(),
                prefix: 0,
                state: ProgressState::Active,
            }),
            changed: Condvar::new(),
        }
    }

    /// 宣言された総サイズを返します。
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// 先頭から連続して到着済みのバイト数を返します。
    pub fn prefix_len(&self) -> u64 {
        self.lock().prefix
    }

    /// アップロードが正常完了したかを返します。
    pub fn is_finished(&self) -> bool {
        self.lock().state == ProgressState::Finished
    }

    /// バイト範囲 `[offset, offset + len)` の到着を記録し、待機中の読みを起こします。
    ///
    /// 範囲は総サイズで切り取られ、既存範囲と重なる分はマージされます。
    /// 接頭辞は連続した範囲でのみ伸びます。
    pub fn record_range(&self, offset: u64, len: u64) {
        if len == 0 || offset >= self.total_size {
            return;
        }
        let end = offset.saturating_add(len).min(self.total_size);
        let mut inner = self.lock();
        insert_range(&mut inner.ranges, offset..end);
        inner.prefix = inner
            .ranges
            .first()
            .filter(|first| first.start == 0)
            .map(|first| first.end)
            .unwrap_or(0);
        drop(inner);
        self.changed.notify_all();
    }

    /// アップロードの正常完了を知らせます。
    ///
    /// 以後、接頭辞を読み尽くした読みは EOF を返します。
    pub fn finish(&self) {
        let mut inner = self.lock();
        if inner.state == ProgressState::Active {
            inner.state = ProgressState::Finished;
        }
        drop(inner);
        self.changed.notify_all();
    }

    /// アップロードの中断を知らせます。
    ///
    /// 待機中の読みはエラーで目覚めます。完了済みの場合は何もしません。
    pub fn abort(&self) {
        let mut inner = self.lock();
        if inner.state == ProgressState::Active {
            inner.state = ProgressState::Aborted;
        }
        drop(inner);
        self.changed.notify_all();
    }

    /// `pos` のバイトが読めるようになるか、状態が確定するまで待機します。
    ///
    /// アップロード側が黙って消えた場合に読み側が永久に止まらないよう、
    /// 待機には全体の上限を設け、超過したら中断として目覚めさせます。
    fn wait_for(&self, pos: u64) -> WaitOutcome {
        const MAX_WAIT: Duration = Duration::from_secs(600);
        let deadline = Instant::now() + MAX_WAIT;
        let mut inner = self.lock();
        loop {
            if inner.state == ProgressState::Aborted {
                return WaitOutcome::Aborted;
            }
            if inner.prefix > pos {
                return WaitOutcome::Readable(inner.prefix);
            }
            if inner.state == ProgressState::Finished {
                return WaitOutcome::Eof;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return WaitOutcome::Aborted;
            }
            let (guard, _) = self
                .changed
                .wait_timeout(inner, remaining.min(Duration::from_secs(5)))
                .unwrap_or_else(|e| e.into_inner());
            inner = guard;
        }
    }

    fn lock(&self) -> MutexGuard<'_, ProgressInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// [`UploadProgress::wait_for`] の結果です。
enum WaitOutcome {
    /// 指定位置を含む接頭辞まで読めます（値は接頭辞の長さ）。
    Readable(u64),
    /// 完了済みで読み尽くしました。
    Eof,
    /// アップロードが中断されました。
    Aborted,
}

/// ソート済み範囲列へ新しい範囲をマージしながら挿入します。
fn insert_range(ranges: &mut Vec<Range<u64>>, new: Range<u64>) {
    let mut start = new.start;
    let mut end = new.end;
    let first_overlapping = ranges.partition_point(|r| r.end < start);
    let mut idx = first_overlapping;
    while idx < ranges.len() && ranges[idx].start <= end {
        start = start.min(ranges[idx].start);
        end = end.max(ranges[idx].end);
        idx += 1;
    }
    ranges.splice(first_overlapping..idx, [start..end]);
}

/// アップロード中のファイルを読み取る `Read + Seek` ビューです。
///
/// 現在位置のバイトが未到着なら到着までブロックし、完了前に EOF を
/// 返すことはありません。シークは最終的な総サイズを基準に論理位置だけを
/// 動かし、接頭辞の外へ進んだ場合は次の読みで到着を待ちます。
pub struct GrowingFile {
    file: File,
    pos: u64,
    progress: Arc<UploadProgress>,
}

impl GrowingFile {
    /// 部分ファイルとそのアップロード進捗からビューを作ります。
    pub fn new(file: File, progress: Arc<UploadProgress>) -> Self {
        Self {
            file,
            pos: 0,
            progress,
        }
    }
}

impl Read for GrowingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let prefix = match self.progress.wait_for(self.pos) {
            WaitOutcome::Readable(prefix) => prefix,
            WaitOutcome::Eof => return Ok(0),
            WaitOutcome::Aborted => {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "asset upload was aborted",
                ));
            }
        };
        let want = ((buf.len() as u64).min(prefix - self.pos)) as usize;
        let mut got = 0usize;
        while got < want {
            let n = read_at(&self.file, &mut buf[got..want], self.pos + got as u64)?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got == 0 {
            // 接頭辞の内側なのにファイルから読めないのは、書き込み側との不整合。
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "uploaded data is missing from the partial file",
            ));
        }
        self.pos += got as u64;
        Ok(got)
    }
}

impl Seek for GrowingFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target: i128 = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(off) => self.pos as i128 + off as i128,
            // End は最終的な総サイズ基準。アップロード中でも全体サイズを報告する。
            SeekFrom::End(off) => self.progress.total_size() as i128 + off as i128,
        };
        if target < 0 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        self.pos = target as u64;
        Ok(self.pos)
    }
}

#[cfg(unix)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::unix::fs::FileExt;
    file.read_at(buf, offset)
}

#[cfg(windows)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::windows::fs::FileExt;
    file.seek_read(buf, offset)
}

#[cfg(not(any(unix, windows)))]
fn read_at(_file: &File, _buf: &mut [u8], _offset: u64) -> io::Result<usize> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "positional reads are not supported on this platform",
    ))
}

#[cfg(test)]
#[path = "../../tests/unit/decode/growing_tests.rs"]
mod tests;
