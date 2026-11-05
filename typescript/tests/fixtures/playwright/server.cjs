const http = require("node:http");

const port = Number(process.env.DX_PLAYWRIGHT_PORT);
http
  .createServer((_request, response) => {
    response.setHeader("content-type", "text/html");
    response.end("<!doctype html><title>hello</title><h1>Hello from rules_dx</h1>");
  })
  .listen(port, "127.0.0.1");
