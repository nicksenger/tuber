export function __tuber_tauri() {
    let message_channel = new MessageChannel();
    let [port1, port2] = [message_channel.port1, message_channel.port2];
    const channel = new __TAURI__.core.Channel();
    channel.onmessage = (data) => {
        let val = new Uint8Array(data);
        port2.postMessage(val, [val.buffer]);
    };
    port2.onmessage = (event) => {
        __TAURI__.core.invoke('plugin:tuber|rx', event.data);
    };
    __TAURI__.core.invoke('plugin:tuber|tx', { channel });

    return port1;
}
