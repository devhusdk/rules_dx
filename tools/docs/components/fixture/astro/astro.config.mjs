import fs from "node:fs";

fs.writeFileSync("astro_config_loaded.marker", "loaded");
export default {};
