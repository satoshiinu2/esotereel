pub trait NetworkHandler: Send + Sync + 'static {
    // Request or Response
    type Packet: Send + Sync + 'static;
    // Server or Client State
    type State: Send + Sync + 'static;
}
