use std::sync::{Arc, Mutex, RwLock};

use esotereel_lib::requests::Request;
use esotereel_lib::responces::Response;
use rkyv::{AlignedVec, check_archived_root};
use tokio::io::{AsyncReadExt, AsyncWriteExt, split};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::state::ClientState;
use crate::{ON_CONNECTED_CALLBACKS, on_responce_recveve};

type ClientSender = mpsc::UnboundedSender<AlignedVec>;

pub type OnConnectedFn = extern "C" fn();

pub struct ClientNetworkHandler {
    tx: RwLock<Option<ClientSender>>,
}

impl ClientNetworkHandler {
    pub fn new() -> Self {
        Self {
            tx: RwLock::new(None),
        }
    }
    pub async fn run(
        self: Arc<Self>,
        state: Arc<Mutex<ClientState>>,
        addr: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(addr).await?;
        let (mut reader, mut writer) = split(stream);

        // 送信用のチャンネルを作成
        let (tx, mut rx) = mpsc::unbounded_channel::<AlignedVec>();

        // 送信チャンネルを保存
        if let Ok(mut guard) = self.tx.write() {
            *guard = Some(tx);
        }

        self.on_connected();

        let instance = Arc::clone(&self);

        // A. 送信専用ループ（mpsc -> TCP）
        let send_task = tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                let size = (data.len() as u32).to_le_bytes();
                if writer.write_all(&size).await.is_err() {
                    break;
                }
                if writer.write_all(&data).await.is_err() {
                    break;
                }
                let _ = writer.flush().await;
            }
        });

        // B. 受信専用ループ（TCP -> HandleResponse）
        // ここで spawn せず、接続が維持されている間はこの関数をブロックする
        let mut size_buf = [0u8; 4];
        while reader.read_exact(&mut size_buf).await.is_ok() {
            let size = u32::from_le_bytes(size_buf) as usize;
            let mut buf = vec![0u8; size];
            if reader.read_exact(&mut buf).await.is_err() {
                break;
            }

            self.parse_and_handle_responce(&state, &buf);
        }

        // クリーンアップ
        log::info!("Client: Server disconnected");
        send_task.abort(); // 送信ループを強制終了
        if let Ok(mut guard) = instance.tx.write() {
            *guard = None; // 送信チャンネルをクリア
        }

        Ok(())
    }

    fn parse_and_handle_responce(&self, state: &Arc<Mutex<ClientState>>, bytes: &Vec<u8>) {
        match check_archived_root::<Response>(bytes) {
            Ok(archived_req) => {
                if let Err(e) = on_responce_recveve(archived_req, state) {
                    log::error!("Handler Error: {:?}", e);
                }
            }
            Err(e) => log::error!("Invalid data format: {:?}", e),
        }
    }

    pub fn send(&self, request: &Request) {
        let bytes = rkyv::to_bytes::<_, 1024>(request).unwrap();
        self.send_bytes(bytes);
    }

    fn send_bytes(&self, bytes: AlignedVec) {
        if let Ok(guard) = self.tx.read() {
            if let Some(tx) = guard.as_ref() {
                if let Err(_) = tx.send(bytes) {
                    log::error!("Failed to send to server: channel closed");
                }
            } else {
                log::warn!(
                    "Dropped request: Client transmitter is not ready. (Connection might not be established yet)"
                );
            }
        }
    }

    fn on_connected(&self) {
        if let Some(cb) = ON_CONNECTED_CALLBACKS.get() {
            cb();
        }
    }
}
