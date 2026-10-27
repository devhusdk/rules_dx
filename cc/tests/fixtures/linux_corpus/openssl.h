// OpenSSL shape stub with declared build tools.
#pragma once

// The canonical upstream build needs perl plus declared tools; ambient
// host-tool discovery stays rejected. This stub proves the declared-input
// C shape.
#ifdef __cplusplus
extern "C" {
#endif

int dx_openssl_version(void);
int dx_openssl_digest(const char *input, char out_hex[65]);

#ifdef __cplusplus
}
#endif
