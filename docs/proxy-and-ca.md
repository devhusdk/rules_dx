# Corporate proxy and custom CA

This page helps you download through a company proxy and trust a company certificate authority (CA). Each download path has its own settings, so set the ones for the paths you use.

## A typical Linux or macOS setup

Most setups need the proxy in the environment and the company CA in two places.

```sh
export HTTPS_PROXY=http://proxy.example.com:3128
export NO_PROXY=localhost,127.0.0.1
export CURL_CA_BUNDLE=/etc/ssl/company-ca.pem
export SSL_CERT_FILE=/etc/ssl/company-ca.pem
```

Then import the CA into a Java trust store for Bazel, and install it in the OS trust store for deploy uploads. The sections below show each step.

## Which setting each download reads

| Download | Proxy | CA trust |
| --- | --- | --- |
| Bazel | `HTTPS_PROXY`, `NO_PROXY` | Java trust store |
| `tools/dx` | curl or wget settings | curl or wget settings |
| `tools/dx.cmd` | PowerShell and Windows settings | Windows certificate store |
| `dx` shim | `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` | `SSL_CERT_FILE`, else OS trust store |
| `dx update` and lock tools | `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` | `SSL_CERT_FILE`, else OS trust store |
| Deploy uploads | `HTTPS_PROXY`, `NO_PROXY` | OS trust store |

## Bazel

Bazel reads the proxy from `HTTPS_PROXY` and `NO_PROXY`.

Bazel runs on Java, so it trusts the Java trust store. To add a company CA, import it into a trust store and point Bazel at it in `.bazelrc`. `startup` lines apply when the Bazel server starts.

```text
startup --host_jvm_args=-Djavax.net.ssl.trustStore=/etc/ssl/company-truststore.jks
startup --host_jvm_args=-Djavax.net.ssl.trustStorePassword=changeit
```

To fetch through a mirror, point Bazel at a downloader config in `.bazelrc`.

```text
common --downloader_config=downloader.cfg
```

The file rewrites URLs.

```text
rewrite github.com/(.*) mirror.example.com/github/$1
```

## tools/dx

`tools/dx` downloads the pinned `dx` binary with `curl`, or with `wget` when `curl` is missing. Both read `HTTPS_PROXY` and `NO_PROXY`.

Add a company CA with the tool's own setting. For curl:

```sh
export CURL_CA_BUNDLE=/etc/ssl/company-ca.pem
```

For wget, set `ca_certificate` in `~/.wgetrc`.

When the download fails, `tools/dx` prints why on stderr and runs your command through `bazel run @rules_dx//:dx` instead.

## tools/dx.cmd

`tools/dx.cmd` downloads with PowerShell `Invoke-WebRequest`. It uses the Windows proxy settings and the Windows certificate store, so install the company CA there.

When the proxy asks for NTLM or Kerberos sign-in, the download answers with your Windows credentials. A proxy that needs basic auth still needs `HTTPS_PROXY` or another route.

When the download fails, `tools/dx.cmd` runs your command through Bazel instead.

## The dx shim

The shim reads `HTTPS_PROXY` for `https://` URLs, `HTTP_PROXY` for `http://` URLs, and `NO_PROXY`. `ALL_PROXY` covers both when the others are unset. `NO_PROXY` entries cover the host and its subdomains. The shim reaches `http://` URLs through the proxy with `CONNECT` to port 80, so the proxy must allow that, or list the host in `NO_PROXY`.

The shim checks certificates with the OS trust store. To use a PEM bundle instead, set `SSL_CERT_FILE`. The bundle replaces the OS store, so it must also hold the public CAs your downloads need.

```sh
export SSL_CERT_FILE=/etc/ssl/company-ca.pem
```

## dx update and lock tools

`dx update` and the lock tools that fetch from registries read the same settings as the shim.

## Deploy uploads

Deploy uploads read `HTTPS_PROXY` and `NO_PROXY`, and check certificates with the OS trust store.
