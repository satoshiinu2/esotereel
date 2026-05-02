use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt, split};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use esotereel_lib::{
    CLIENT_ALL, ServerState,
    requests::{parse_and_handle_request, set_request_callbacks},
    set_send_response_callback,
};

use crate::requests::on_request_receive;

type ClientSender = mpsc::UnboundedSender<Vec<u8>>;

pub static INSTANCE: RwLock<Option<Arc<ServerNetworkHandler>>> = RwLock::new(None);

pub struct ServerNetworkHandler {
    pub app_state: Arc<ServerState>,
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

    pub fn new(app_state: Arc<ServerState>) -> Self {
        set_request_callbacks(on_request_receive);
        set_send_response_callback(on_send);
        Self {
            app_state,
            clients: RwLock::new(HashMap::new()),
        }
    }

    pub async fn run(self: Arc<Self>, addr: &str) -> Result<(), std::io::Error> {
        // グローバルインスタンスを登録 (Cコールバック用)
        *INSTANCE.write().unwrap() = Some(self.clone());

        let listener = TcpListener::bind(addr).await?;
        log::info!("Server listening on {}", addr);

        let mut client_id_counter = 0;

        loop {
            let (stream, _) = listener.accept().await?;
            let client_id = client_id_counter;
            client_id_counter += 1;

            let app_state = self.app_state.clone();
            let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

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

                        if let Err(e) = parse_and_handle_request(&buf, client_id, &app_state) {
                            log::error!("Handler Error: {:?}", e);
                        }
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
}

extern "C" fn on_send(client_id: u32, ptr: *const u8, len: usize) {
    let data = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();

    if let Some(instance) = ServerNetworkHandler::get_instance() {
        if let Ok(clients) = instance.clients.read() {
            if client_id == CLIENT_ALL {
                // 全クライアントへの配信
                for tx in clients.values() {
                    let _ = tx.send(data.clone());
                }
            } else {
                if let Some(tx) = clients.get(&client_id) {
                    if let Err(_) = tx.send(data) {
                        log::error!("Failed to send to client {}: channel closed", client_id);
                    }
                }
            }
        }
    }
}
