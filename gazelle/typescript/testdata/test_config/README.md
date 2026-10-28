# Test config setup files

Verifies that literal `setupFiles` and `globalSetup` entries in a Vitest
config become deps of the config's project. Dynamic entries add no deps.
