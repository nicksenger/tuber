Error.stackTraceLimit = 50;
globalThis.onerror = console.error;

let x = undefined;
globalThis.onmessage = async message => {
    if (x == undefined) {
        x = null;
        await initWithProgress(message.data);
    }
};

async function initWithProgress([messagePort, name, origin]) {
    const response = fetch(`${origin}/${name}_bg.wasm`)
        .then((response) => {
            const reader = response.body.getReader();
            const headers = response.headers;
            const status = response.status;
            const statusText = response.statusText;

            const stream = new ReadableStream({
                start(controller) {
                    function push() {
                        reader.read().then(({ done, value }) => {
                            if (done) {
                                controller.close();
                                return;
                            }

                            controller.enqueue(value);
                            push();
                        });
                    }

                    push();
                },
            });

            return {
                stream, init: {
                    headers, status, statusText
                }
            };
        })
        .then(({ stream, init }) =>
            new Response(stream, init),
        );

    import(`${origin}/${name}.js`).then(init => {
        init.default(response)
            .then((worker) => {
                globalThis[`__tuber_message_port_${name}`] = messagePort;
                (worker[`__tuber_worker_entrypoint_${name}`])();
            }, (reason) => {
                return reason;
            });
    }, (reason) => {
        return reason;
    });
}
