# Split binary layout

The library package stays fully generated while the thin `cc_binary` lives
in the excluded `main/` child directory, so generation never rewrites
handwritten binary ownership. Dropping the exclusion fails generation
with the main-source guard instead.
