use esotereel_lib::requests::Request;
use esotereel_lib::responces::Response;
use rkyv::{AlignedVec, check_archived_root};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt, split};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use esotereel_lib::ServerState;

use crate::OnServerReadyFn;
use crate::requests::on_request_receive;

type ClientSender = mpsc::UnboundedSender<AlignedVec>;

pub static INSTANCE: RwLock<Option<Arc<ServerNetworkHandler>>> = RwLock::new(None);

pub struct ServerNetworkHandler {
    pub app_state: Arc<Mutex<ServerState>>,
    clients: RwLock<HashMap<u32, ClientSender>>,
}

impl ServerNetworkHandler {
    pub fn get_instance() -> Option<Arc<ServerNetworkHandler>> {
        if let Ok(instance_guard) = INSTANCE.read() {
            if let Some(instance) = instance_guard.as_ref() {
                return Some(instance.clone());
            }
        }
        None
    }

    pub fn new(app_state: Arc<Mutex<ServerState>>) -> Self {
        Self {
            app_state,
            clients: RwLock::new(HashMap::new()),
        }
    }

    pub async fn run(
        self: Arc<Self>,
        addr: &str,
        on_server_ready: Option<OnServerReadyFn>,
    ) -> Result<(), std::io::Error> {
        // グローバルインスタンスを登録 (Cコールバック用)
        *INSTANCE.write().unwrap() = Some(self.clone());

        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => {
                if let Some(f) = on_server_ready {
                    f(true);
                }
                Ok(l)
            }
            Err(e) => {
                if let Some(f) = on_server_ready {
                    f(false);
                }
                Err(e)
            }
        }?;

        log::info!("Server listening on {}", addr);

        let mut client_id_counter = 0;

        loop {
            let (stream, _) = listener.accept().await?;
            let client_id = client_id_counter;
            client_id_counter += 1;

            let (tx, mut rx) = mpsc::unbounded_channel::<AlignedVec>();

            // CLIENTSに登録
            let mut guard = self.clients.write().unwrap();
            guard.insert(client_id, tx);

            let instance = self.clone();

            // クライアントごとのメインタスク
            tokio::spawn(async move {
                log::info!("Client {} connected", client_id);

                let (mut reader, mut writer) = split(stream);

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

                // B. 受信専用ループ（TCP -> HandleRequest）

                tokio::spawn(async move {
                    let mut size_buf = [0u8; 4];
                    while reader.read_exact(&mut size_buf).await.is_ok() {
                        let size = u32::from_le_bytes(size_buf) as usize;
                        let mut buf = vec![0u8; size];
                        if reader.read_exact(&mut buf).await.is_err() {
                            break;
                        }

                        instance.parse_and_handle_request(&buf, client_id);
                    }

                    // クリーンアップ
                    log::info!("Client {} disconnected", client_id);
                    send_task.abort(); // 送信タスクを終了させる

                    if let Ok(mut guard) = instance.clients.write() {
                        guard.remove(&client_id);
                    }
                });
            });
        }
    }

    fn parse_and_handle_request(self: &Arc<Self>, bytes: &Vec<u8>, client_id: u32) {
        match check_archived_root::<Request>(bytes) {
            Ok(archived_req) => {
                if let Err(e) = on_request_receive(archived_req, client_id, self) {
                    log::error!("Handler Error: {:?}", e);
                }
            }
            Err(e) => log::error!("Invalid data format: {:?}", e),
        }
    }

    pub fn send(&self, client_id: u32, request: &Response) {
        let bytes = rkyv::to_bytes::<_, 1024>(request).unwrap();
        self.send_bytes(client_id, bytes);
    }

    pub fn send_all(&self, request: &Response) {
        let bytes = rkyv::to_bytes::<_, 1024>(request).unwrap();
        self.send_bytes_all(bytes);
    }

    fn send_bytes_all(&self, bytes: AlignedVec) {
        if let Ok(clients) = self.clients.read() {
            for (client_id, tx) in clients.iter() {
                if let Err(_) = tx.send(bytes.clone()) {
                    log::error!("Failed to send to client {}: channel closed", client_id);
                }
            }
        }
    }

    fn send_bytes(&self, client_id: u32, bytes: AlignedVec) {
        if let Ok(clients) = self.clients.read() {
            if let Some(tx) = clients.get(&client_id) {
                if let Err(_) = tx.send(bytes) {
                    log::error!("Failed to send to client {}: channel closed", client_id);
                }
            } else {
                log::error!("Failed to send to client {}: not found", client_id);
            }
        }
    }
}
