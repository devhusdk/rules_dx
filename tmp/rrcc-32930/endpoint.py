#!/usr/bin/env python3
"""A grpcs:// endpoint for Bazel's remote cache that can stop answering.

  endpoint.py --port P
      Accept TCP and never send a byte, so every TLS handshake stalls (a cache dead from the start).
  endpoint.py --port P --upstream HOST:PORT --cert C --key K [--on-stall hang|freeze|cut]
      Terminate TLS with C/K and forward HTTP/2 to a plaintext gRPC cache (e.g. bazel-remote) until
      SIGUSR1 arrives, then stall: new connections never get a TLS ServerHello, and open
      connections either
        hang:   stay open and answer HTTP/2 PINGs, but nothing else gets through, like a proxy in
                front of a backend that stopped answering (default);
        freeze: stay open, but nothing at all comes back, PINGs included;
        cut:    are reset.

Each connection is logged to stderr with a timestamp; each one is a TLS handshake attempt.
"""
import argparse
import asyncio
import signal
import ssl
import sys
import time

T0 = time.monotonic()
PREFACE = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
PING, ACK = 6, 1


def log(msg):
    print(f"[endpoint +{time.monotonic() - T0:7.2f}s] {msg}", file=sys.stderr, flush=True)


async def frames(reader, preface):
    """Yields the client preface (if any) and then whole HTTP/2 frames, until EOF."""
    try:
        if preface:
            yield await reader.readexactly(len(preface))
        while True:
            head = await reader.readexactly(9)
            yield head + await reader.readexactly(int.from_bytes(head[:3]))
    except (asyncio.IncompleteReadError, ConnectionError, OSError):
        return


class Endpoint:
    def __init__(self, upstream, tls, on_stall):
        self.upstream = upstream
        self.tls = tls
        self.on_stall = on_stall
        self.stalling = upstream is None
        self.open = set()
        self.count = 0
        self.pings = 0

    def stall(self):
        if self.stalling:
            return
        self.stalling = True
        verb = {"hang": "hanging", "freeze": "freezing", "cut": "cutting"}[self.on_stall]
        log(f"now stalling; {verb} {len(self.open)} open connection(s)")
        if self.on_stall == "cut":
            for w in list(self.open):
                w.transport.abort()

    async def handle(self, reader, writer):
        self.count += 1
        n = self.count
        if self.stalling:
            log(f"conn {n}: accepted, stalling (no TLS ServerHello will be sent)")
            try:
                while await reader.read(65536):
                    pass
            except (ConnectionError, OSError):
                pass
            log(f"conn {n}: closed by client")
            writer.close()
            return
        try:
            await writer.start_tls(self.tls)
        except (ssl.SSLError, ConnectionError, OSError) as e:
            log(f"conn {n}: TLS handshake failed: {e!r}")
            writer.close()
            return
        up_reader, up_writer = await asyncio.open_connection(*self.upstream)
        log(f"conn {n}: forwarding")
        self.open.add(writer)

        async def pipe(src, dst, from_client):
            # Whole frames only, so a PING ACK written during a hang never splits another frame.
            # The client preface can't be taken for a PING: its fourth byte is a space.
            async for f in frames(src, PREFACE if from_client else b""):
                if not self.stalling:
                    dst.write(f)
                    await dst.drain()
                elif self.on_stall == "hang" and from_client and f[3] == PING and not f[4] & ACK:
                    writer.write(f[:4] + bytes([ACK]) + f[5:])
                    self.pings += 1
                    log(f"conn {n}: answered PING #{self.pings}")
            dst.transport.abort()

        await asyncio.gather(pipe(reader, up_writer, True), pipe(up_reader, writer, False))
        self.open.discard(writer)
        log(f"conn {n}: closed")


async def main():
    p = argparse.ArgumentParser()
    p.add_argument("--port", type=int, required=True)
    p.add_argument("--upstream", help="HOST:PORT of a plaintext gRPC cache")
    p.add_argument("--cert", help="TLS certificate (PEM), with --upstream")
    p.add_argument("--key", help="TLS private key (PEM), with --upstream")
    p.add_argument("--on-stall", choices=("hang", "freeze", "cut"), default="hang",
                   help="what happens to open connections when the stall starts")
    a = p.parse_args()
    upstream = tls = None
    if a.upstream:
        host, _, port = a.upstream.rpartition(":")
        upstream = (host, int(port))
        tls = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
        tls.load_cert_chain(a.cert, a.key)
        tls.set_alpn_protocols(["h2"])
    ep = Endpoint(upstream, tls, a.on_stall)
    asyncio.get_running_loop().add_signal_handler(signal.SIGUSR1, ep.stall)
    server = await asyncio.start_server(ep.handle, "127.0.0.1", a.port)
    log(f"listening on 127.0.0.1:{a.port}, "
        + (f"forwarding to {a.upstream}" if upstream else "stalling"))
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
