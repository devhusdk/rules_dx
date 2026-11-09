# Source-Only TypeScript Generation

Verifies conventional one-source project generation without authoritative
manifests or generated sidecars. A declaration file with no same-stem source and no owning rule gets a
declaration-only `typescript_library` that lists it in `declaration_srcs`.
