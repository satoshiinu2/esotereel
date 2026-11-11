//! Core サーバーのスモークテスト用クライアント。
//!
//! `scripts/smoke-test.sh` から起動された core サーバー (127.0.0.1:12345) に
//! 接続し、NewProject -> FetchClipsInRange -> Command(AddLayer) の順に
//! リクエストを送り、対応するレスポンス (ProjectMeta / UpdateClip /
//! UpdateLayer) が制限時間内に返ることを確認する。
//! タイムアウトした場合は異常終了する（= フリーズ/デッドロック検出）。

use std::{collections::VecDeque, time::Duration};

use esotereel_lib::{
    project::command::CommandRequest,
    requests::Request,
    responces::{ArchivedResponse, Response},
};
use rkyv::{
    AlignedVec, check_archived_root,
    ser::{Serializer, serializers::AllocSerializer},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
    time::timeout,
};

const RECV_TIMEOUT: Duration = Duration::from_secs(10);

fn serialize(req: &Request) -> AlignedVec {
    let mut serializer = AllocSerializer::<8192>::default();
    serializer
        .serialize_value(req)
        .expect("request serialization must succeed");
    serializer.into_serializer().into_inner()
}

async fn send(writer: &mut tokio::net::tcp::OwnedWriteHalf, req: &Request) {
    let bytes = serialize(req);
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .await
        .expect("send length");
    writer.write_all(&bytes).await.expect("send payload");
    writer.flush().await.expect("flush");
}

/// 次のレスポンスが述語を満たすまで読み進める。タイムアウトは失敗。
async fn wait_for(
    rx: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    stage: &str,
    mut pred: impl FnMut(&ArchivedResponse) -> bool,
) {
    let fut = async {
        while let Some(buf) = rx.recv().await {
            let archived = check_archived_root::<Response>(&buf).expect("valid response frame");
            if pred(archived) {
                return;
            }
        }
        panic!("connection closed while waiting for {stage}");
    };
    timeout(RECV_TIMEOUT, fut)
        .await
        .unwrap_or_else(|_| panic!("timeout waiting for {stage}: server unresponsive"));
    println!("smoke: {stage} OK");
}

#[tokio::main]
async fn main() {
    // サーバー起動を待ちながら接続する
    let mut stream = None;
    for _ in 0..50 {
        match TcpStream::connect("127.0.0.1:12345").await {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
    let stream = stream.expect("server did not start listening on 127.0.0.1:12345");
    println!("smoke: connected");

    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        let mut len_buf = [0u8; 4];
        while reader.read_exact(&mut len_buf).await.is_ok() {
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            if reader.read_exact(&mut buf).await.is_err() {
                break;
            }
            if tx.send(buf).is_err() {
                break;
            }
        }
    });

    // 1. NewProject -> ProjectMeta (タイムライン1つ)
    send(&mut writer, &Request::NewProject).await;
    wait_for(&mut rx, "ProjectMeta after NewProject", |resp| match resp {
        ArchivedResponse::ProjectMeta { timelines } => timelines.len() == 1,
        _ => false,
    })
    .await;

    // 2. FetchClipsInRange -> UpdateClip
    send(
        &mut writer,
        &Request::FetchClipsInRange {
            timeline_id: 0,
            range: i64::MIN..i64::MAX,
        },
    )
    .await;
    wait_for(&mut rx, "UpdateClip after FetchClipsInRange", |resp| {
        matches!(resp, ArchivedResponse::UpdateClip { .. })
    })
    .await;

    // 3. Command(AddLayer) -> dirty 伝播で UpdateLayer が届く
    let mut commands = VecDeque::new();
    commands.push_back((
        0,
        CommandRequest::AddLayer {
            parent_folder_id: None,
            insert_index: None,
            name: "smoke-layer".to_string(),
        },
    ));
    send(&mut writer, &Request::Command { commands }).await;
    wait_for(&mut rx, "UpdateLayer after AddLayer command", |resp| match resp {
        ArchivedResponse::UpdateLayer { layers, .. } => layers
            .iter()
            .any(|layer| layer.name.as_str() == "smoke-layer"),
        _ => false,
    })
    .await;

    println!("smoke: PASS (server stayed responsive through the whole sequence)");
}
