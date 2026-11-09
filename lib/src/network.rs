/// ネットワークハンドラのトレイトです。
///
/// リクエストまたはレスポンスのパケット型と、サーバー/クライアントの状態型を
/// 関連型として持ち、プロトコルの送受信を抽象化します。
pub trait NetworkHandler: Send + Sync + 'static {
    /// 送受信するパケット（リクエストまたはレスポンス）の型です。
    type Packet: Send + Sync + 'static;
    /// サーバーまたはクライアントの状態の型です。
    type State: Send + Sync + 'static;
}
