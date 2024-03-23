export default function initializer() {
    return {
        onStart: () => {
        },
        onProgress: ({ current, total }) => {
        },
        onComplete: () => {
        },
        onSuccess: (wasm) => {
            setTimeout(() => {
                wasm.entrypoint();
            }, 0);
        },
        onFailure: (error) => {
        }
    }
};
