#include <windows.h>

#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <wincrypt.h>

static int hex_value(wchar_t c) {
  if (c >= L'0' && c <= L'9')
    return c - L'0';
  if (c >= L'a' && c <= L'f')
    return c - L'a' + 10;
  if (c >= L'A' && c <= L'F')
    return c - L'A' + 10;
  return -1;
}

static void write_key(FILE *out, const wchar_t *hex) {
  BYTE hash[20];
  if (wcslen(hex) != 40) {
    fputws(L"key=none\n", out);
    return;
  }
  for (int i = 0; i < 20; i++) {
    int hi = hex_value(hex[2 * i]);
    int lo = hex_value(hex[2 * i + 1]);
    if (hi < 0 || lo < 0) {
      fputws(L"key=none\n", out);
      return;
    }
    hash[i] = (BYTE)(hi * 16 + lo);
  }
  HCERTSTORE store = CertOpenStore(
      CERT_STORE_PROV_SYSTEM_W, 0, 0,
      CERT_SYSTEM_STORE_CURRENT_USER | CERT_STORE_OPEN_EXISTING_FLAG, L"MY");
  if (store == NULL) {
    fputws(L"key=none\n", out);
    return;
  }
  CRYPT_HASH_BLOB blob = {sizeof(hash), hash};
  PCCERT_CONTEXT cert =
      CertFindCertificateInStore(store, X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                                 0, CERT_FIND_SHA1_HASH, &blob, NULL);
  DWORD size = 0;
  if (cert != NULL && CertGetCertificateContextProperty(
                          cert, CERT_KEY_PROV_INFO_PROP_ID, NULL, &size)) {
    CRYPT_KEY_PROV_INFO *info = (CRYPT_KEY_PROV_INFO *)malloc(size);
    if (info != NULL && CertGetCertificateContextProperty(
                            cert, CERT_KEY_PROV_INFO_PROP_ID, info, &size)) {
      fwprintf(out, L"key=%ls|%ls|%lu\n", info->pwszProvName,
               info->pwszContainerName, (unsigned long)info->dwProvType);
    } else {
      fputws(L"key=none\n", out);
    }
    free(info);
  } else {
    fputws(L"key=none\n", out);
  }
  if (cert != NULL)
    CertFreeCertificateContext(cert);
  CertCloseStore(store, 0);
}

int wmain(int argc, wchar_t **argv) {
  wchar_t path[MAX_PATH * 4];
  wchar_t code[16];
  DWORD n = GetEnvironmentVariableW(L"DX_MSIX_ARGV_FILE", path,
                                    (DWORD)(sizeof(path) / sizeof(path[0])));
  if (n > 0 && n < sizeof(path) / sizeof(path[0])) {
    FILE *out = _wfopen(path, L"a, ccs=UTF-8");
    if (out != NULL) {
      const wchar_t *sha1 = NULL;
      for (int i = 0; i < argc; i++) {
        fwprintf(out, L"%ls\n", argv[i]);
        if (wcscmp(argv[i], L"/sha1") == 0 && i + 1 < argc)
          sha1 = argv[i + 1];
      }
      if (sha1 != NULL)
        write_key(out, sha1);
      fputws(L"--\n", out);
      fclose(out);
    }
  }
  n = GetEnvironmentVariableW(L"DX_MSIX_ARGV_EXIT", code,
                              (DWORD)(sizeof(code) / sizeof(code[0])));
  return n > 0 && n < sizeof(code) / sizeof(code[0]) ? _wtoi(code) : 0;
}
