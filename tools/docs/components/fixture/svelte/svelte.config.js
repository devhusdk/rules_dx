import fs from "node:fs";

fs.writeFileSync("svelte_config_loaded.marker", "loaded");
export default {};
