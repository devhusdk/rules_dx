"""Pinned Swift toolchain archives."""

SWIFT_ARCHIVES = {
    "6.4.1": {
        "aarch64": struct(
            platform = "ubuntu22.04-aarch64",
            sha256 = "5d50538c0500ff1b253e799f3e48f04db0547856aef76905c85e5bdfe01e6901",
        ),
        "x86_64": struct(
            platform = "ubuntu22.04",
            sha256 = "e9ec2ff05ac0fc89bf2e7cecdee5da4d398898a768d7ee2b86a59d2a7f01c44b",
        ),
    },
}
