import init, { add, greet, shout_greet } from "./wasm_hello_web/wasm_hello_web.js";

export async function report(name: string): Promise<string> {
    await init();
    return [greet(name), shout_greet(name), String(add(1, 2))].join(" ");
}
