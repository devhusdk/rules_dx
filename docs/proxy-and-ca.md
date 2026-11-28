# Corporate proxy and custom CA

This page helps you download through a company proxy and trust a company certificate authority (CA). Each download path has its own settings, so set the ones for the paths you use.

## A typical Linux or macOS setup

Most setups need the proxy in the environment and the company CA in two places.

```sh
export HTTPS_PROXY=http://proxy.example.com:3128
export NO_PROXY=localhost,127.0.0.1
export CURL_CA_BUNDLE=/etc/ssl/company-ca.pem
```

Then import the CA into a Java trust store for Bazel, and install it in the OS trust store, or name it in `SSL_CERT_FILE`, for the `dx` shim and deploy uploads. The sections below show each step.

## Which setting each download reads

| Download | Proxy | CA trust |
| --- | --- | --- |
| Bazel | `HTTPS_PROXY`, `NO_PROXY` | Java trust store |
| `tools/dx` | curl or wget settings | curl or wget settings |
| `tools/dx.cmd` | PowerShell and Windows settings | Windows certificate store |
| `dx` shim | `HTTPS_PROXY`, `NO_PROXY` | OS trust store, or `SSL_CERT_FILE` |
| `dx update` and lock tools | `HTTPS_PROXY`, `NO_PROXY` | OS trust store, or `SSL_CERT_FILE` |
| Deploy uploads | `HTTPS_PROXY`, `NO_PROXY` | OS trust store, or `SSL_CERT_FILE` |

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

When your proxy blocks GitHub source archives, fetch them from the Bazel mirrors instead. Bazel tries the URLs in order and checks each archive's checksum. The last line keeps the GitHub URL.

```text
rewrite (github\.com/[^/]+/[^/]+/archive/.+) mirror.bazel.build/$1
rewrite (github\.com/[^/]+/[^/]+/archive/.+) bcr.cloudflaremirrors.com/$1
rewrite (github\.com/[^/]+/[^/]+/archive/.+) $1
rewrite codeload\.github\.com/([^/]+)/([^/]+)/(tar\.gz|zip)/(.+) mirror.bazel.build/github.com/$1/$2/archive/$4.$3
rewrite codeload\.github\.com/([^/]+)/([^/]+)/(tar\.gz|zip)/(.+) bcr.cloudflaremirrors.com/github.com/$1/$2/archive/$4.$3
rewrite codeload\.github\.com/([^/]+)/([^/]+)/(tar\.gz|zip)/(.+) codeload.github.com/$1/$2/$3/$4
```

## tools/dx

`tools/dx` downloads the pinned `dx` binary with `curl`, or with `wget` when `curl` is missing. Both read `HTTPS_PROXY` and `NO_PROXY`.

Add a company CA with the tool's own setting. For curl:

```sh
export CURL_CA_BUNDLE=/etc/ssl/company-ca.pem
```

For wget, set `ca_certificate` in `~/.wgetrc`.

When the download fails, `tools/dx` prints why on stderr and runs your command through `bazel run @rules_dx//dx/launch:dx` instead.

## tools/dx.cmd

`tools/dx.cmd` downloads with PowerShell `Invoke-WebRequest`. It uses the Windows proxy settings and the Windows certificate store, so install the company CA there.

When the proxy asks for NTLM or Kerberos sign-in, the download answers with your Windows credentials. A proxy that needs basic auth still needs `HTTPS_PROXY` or another route.

When the download fails, `tools/dx.cmd` runs your command through Bazel instead.

## The dx shim

The shim sends `https://` downloads through the proxy in `HTTPS_PROXY`. When that is unset, it uses `ALL_PROXY`, then `HTTP_PROXY`. Hosts in `NO_PROXY` go direct, and an entry covers the host and its subdomains. `http://` downloads, such as a local mirror, always go direct.

The shim checks certificates with the OS trust store. Install the company CA there, and you need no other setting.

To use a PEM bundle instead of the OS store, set `SSL_CERT_FILE`. The bundle replaces the OS store, so it must hold the public CAs as well as the company CA.

```sh
cat /etc/ssl/certs/ca-certificates.crt /etc/ssl/company-ca.pem > ~/.config/dx-ca.pem
export SSL_CERT_FILE=~/.config/dx-ca.pem
```

## dx update and lock tools

`dx update` and the lock tools that fetch from registries read the same settings as the shim.

Lock tools also follow the downloader config that `bazel run` uses. They fetch through its rewrites and honour its `allow` and `block` lines. The lock still records the upstream URLs.

## Deploy uploads

Deploy uploads read the same settings as the shim.
