# Corporate proxy and custom CA

Each download path has its own proxy and CA settings. Set the ones for the paths you use.

| Download | Proxy | CA trust |
| --- | --- | --- |
| Bazel | `HTTPS_PROXY`, `NO_PROXY` | Java trust store |
| `tools/dx` | curl or wget settings | curl or wget settings |
| `tools/dx.bat` | PowerShell and Windows settings | Windows certificate store |
| `dx` shim | `HTTPS_PROXY`, `NO_PROXY` | OS trust store |
| Deploy uploads | `HTTPS_PROXY`, `NO_PROXY` | OS trust store |

## Bazel

Bazel reads the proxy from the environment.

```sh
export HTTPS_PROXY=http://proxy.example.com:3128
export NO_PROXY=localhost,127.0.0.1
```

Bazel trusts the Java trust store. To add a company CA, import it into a trust store and point Bazel at it in `.bazelrc`.

```text
startup --host_jvm_args=-Djavax.net.ssl.trustStore=/etc/ssl/company-truststore.jks
startup --host_jvm_args=-Djavax.net.ssl.trustStorePassword=changeit
```

To fetch through a mirror, rewrite URLs with a downloader config.

```text
common --downloader_config=downloader.cfg
```

```text
rewrite github.com/(.*) mirror.example.com/github/$1
```

## tools/dx

`tools/dx` downloads the pinned binary with `curl`, or `wget` when `curl` is missing. Both read `HTTPS_PROXY` and `NO_PROXY`.

Add a company CA with the tool's own setting.

```sh
export CURL_CA_BUNDLE=/etc/ssl/company-ca.pem
```

wget reads `ca_certificate` from `~/.wgetrc`.

## tools/dx.bat

`tools/dx.bat` downloads with PowerShell `Invoke-WebRequest`. It uses the Windows proxy settings and the Windows certificate store. Install the company CA there.

## The dx shim

The shim reads `HTTPS_PROXY` and `NO_PROXY` and checks certificates with the OS trust store. Install the company CA in the OS store. No extra setting is needed.

## Deploy uploads

Deploy uploads read `HTTPS_PROXY` and `NO_PROXY` and check certificates with the OS trust store.
