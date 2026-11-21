const http = require("node:http");

const server = http.createServer((_request, response) => {
  response.setHeader("content-type", "text/html");
  response.end("<!doctype html><title>hello</title><h1>Hello from rules_dx</h1>");
});
server.listen(0, "127.0.0.1", () => {
  const { port } = server.address();
  console.log(`listening on http://127.0.0.1:${port}`);
});
