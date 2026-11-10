import fs from "node:fs";

const [marker, ...pairs] = process.argv.slice(2);
let failed = false;
for (let i = 0; i < pairs.length; i += 2) {
  if (fs.readFileSync(pairs[i], "utf8").trim() !== "0") {
    process.stderr.write(fs.readFileSync(pairs[i + 1], "utf8"));
    failed = true;
  }
}
if (failed) {
  process.exit(1);
}
fs.writeFileSync(marker, "");
