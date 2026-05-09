use std::sync::{Arc, RwLock};

use esotereel_lib::ClientState;
use esotereel_lib::requests::Request;
use esotereel_lib::responces::Response;
use rkyv::{AlignedVec, check_archived_root};
use tokio::io::{AsyncReadExt, AsyncWriteExt, split};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::{ON_CONNECTED_CALLBACKS, on_responce_recveve};

type ClientSender = mpsc::UnboundedSender<AlignedVec>;

static INSTANCE: RwLock<Option<Arc<ClientNetworkHandler>>> = RwLock::new(None);

pub type OnConnectedFn = extern "C" fn();

pub struct ClientNetworkHandler {
    pub app_state: Arc<ClientState>,
    tx: RwLock<Option<ClientSender>>,
}

impl ClientNetworkHandler {
    pub fn get_instance() -> Option<Arc<ClientNetworkHandler>> {
        if let Ok(instance_guard) = INSTANCE.read() {
            if let Some(instance) = instance_guard.as_ref() {
                return Some(instance.clone());
            }
        }
        None
    }

    pub fn new(app_state: Arc<ClientState>) -> Self {
        Self {
            app_state,
            tx: RwLock::new(None),
        }
    }
    pub async fn run(self: Arc<Self>, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        // グローバルインスタンスを登録 (Cコールバック用)
        if let Ok(mut guard) = INSTANCE.write() {
            *guard = Some(self.clone());
        }

        let stream = TcpStream::connect(addr).await?;
        let (mut reader, mut writer) = split(stream);

        // 送信用のチャンネルを作成
        let (tx, mut rx) = mpsc::unbounded_channel::<AlignedVec>();

        // 送信チャンネルを保存
        if let Ok(mut guard) = self.tx.write() {
            *guard = Some(tx);
        }

        self.on_connected();

        let instance = self.clone();

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

            self.parse_and_handle_responce(&buf);
        }

        // クリーンアップ
        log::info!("Client: Server disconnected");
        send_task.abort(); // 送信ループを強制終了
        if let Ok(mut guard) = instance.tx.write() {
            *guard = None; // 送信チャンネルをクリア
        }

        Ok(())
    }

    fn parse_and_handle_responce(self: &Arc<Self>, bytes: &Vec<u8>) {
        match check_archived_root::<Response>(bytes) {
            Ok(archived_req) => {
                if let Err(e) = on_responce_recveve(archived_req, self) {
                    log::error!("Handler Error: {:?}", e);
                }
            }
            Err(e) => log::error!("Invalid data format: {:?}", e),
        }
    }

    pub fn send(&self, request: Request) {
        let bytes = rkyv::to_bytes::<_, 1024>(&request).unwrap();
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
