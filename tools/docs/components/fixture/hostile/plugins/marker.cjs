// Writes a marker file when loaded. The isolated launcher must never load it.
require("node:fs").writeFileSync("hostile_plugin_loaded.marker", "loaded\n");
module.exports = () => ({ version: 2 });
