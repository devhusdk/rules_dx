import init, { greet, shout_greet } from "./wasm_hello_web/wasm_hello_web.js";

await init();
document.querySelector("#greeting").textContent = greet("browser");
document.querySelector("#shout").textContent = shout_greet("browser");
