"""Pinned Ruby foundation."""

load(":versions.bzl", _RUBY_VERSION = "RUBY_VERSION", _RULES_RUBY_VERSION = "FLOOR_RULES_RUBY")

RULES_RUBY_VERSION = _RULES_RUBY_VERSION
RUBY_VERSION = _RUBY_VERSION

GEM_CHECKSUMS = {
    "ast-2.4.3": "954615157c1d6a382bc27d690d973195e79db7f55e9765ac7c481c60bdb4d383",
    "diff-lcs-1.5.0": "49b934001c8c6aedb37ba19daec5c634da27b318a7a3c654ae979d6ba1929b67",
    "json-2.19.9": "9b9025b7cdddafa38d316eca0b2358488e42d417045c1b90d216a9fefe46b79a",
    "language_server-protocol-3.17.0.6": "5ef2c0c138f8267e1bc631d3328347d354f96724b0af22f2c79516120443b7f0",
    "lint_roller-1.1.0": "2c0c845b632a7d172cb849cc90c1bce937a28c5c8ccccb50dfd46a485003cc87",
    "parallel-1.28.0": "33e6de1484baf2524792d178b0913fc8eb94c628d6cfe45599ad4458c638c970",
    "parser-3.3.12.0": "21a6d7f755d5a24dfbdc6e6b772e4e879a52e7631a88bc5a3a134606052c9828",
    "racc-1.8.1": "4a7f6929691dbec8b5209a0b373bc2614882b55fc5d2e447a21aaa691303d62f",
    "rainbow-3.1.1": "039491aa3a89f42efa1d6dec2fc4e62ede96eb6acd95e52f1ad581182b79bc6a",
    "regexp_parser-2.13.1": "5aedb6b7c35688f51e86eef17a15374dfd287c83f4ca644b2c5f3ec50a33b44b",
    "rspec-3.13.0": "d490914ac1d5a5a64a0e1400c1d54ddd2a501324d703b8cfe83f458337bab993",
    "rspec-core-3.13.0": "557792b4e88da883d580342b263d9652b6a10a12d5bda9ef967b01a48f15454c",
    "rspec-expectations-3.13.0": "621d48c62262f955421eaa418130744760802cad47e781df70dba4d9f897102e",
    "rspec-mocks-3.13.0": "735a891215758d77cdb5f4721fffc21078793959d1f0ee4a961874311d9b7f66",
    "rspec-support-3.13.1": "48877d4f15b772b7538f3693c22225f2eda490ba65a0515c4e7cf6f2f17de70f",
    "rubocop-1.73.2": "35cd1b1365ba97234323fe771abcecd09c9b77098464cd726c76aa7d9bc12b5d",
    "rubocop-ast-1.41.0": "0eeff9ad3743f42823e3c687cbc15f8c552ff63c9f8e3d35fbafe746977c7c0d",
    "rubocop-performance-1.24.0": "e5bd39ff3e368395b9af886927cc37f5892f43db4bd6c8526594352d5b4440b5",
    "ruby-progressbar-1.13.0": "80fc9c47a9b640d6834e0dc7b3c94c9df37f08cb072b7761e4a71e22cff29b33",
    "standard-1.47.0": "b0da6b71d8dec53e760c7216b723e3507ebdcd6962f7ce7c37c016a36c7cc190",
    "standard-custom-1.0.2": "424adc84179a074f1a2a309bb9cf7cd6bfdb2b6541f20c6bf9436c0ba22a652b",
    "standard-performance-1.7.0": "c46a9aef3348c0cfc03053ed9a1c1f73b967f7e4edcdf30dac0101b143897314",
    "unicode-display_width-3.3.0": "4b7aa66a4b11db50f6f7e98411215cd0dd10eaecb665d36eec5bcacb3fa0b613",
    "unicode-emoji-4.3.0": "11c02fa73290378c066bb0562cd4c87d8e1b706fbbe0059ca12746a4244de8ce",
}

# portable-ruby release 3.4.12-1. rules_ruby 0.29.0 ships no checksums for 3.4.12, so
# the root and consumer toolchains pass these (sha256 of the release assets).
PORTABLE_RUBY_CHECKSUMS = {
    "ruby-3.4.12.arm64_darwin.tar.gz": "690e1eb8bb715a136029a62a60de50d5b463e275666a81246ca09a1e8f8ac1c2",
    "ruby-3.4.12.arm64_linux.tar.gz": "e36011c92255185e2e76f409cc1011bb652425bec6bb6e2800aadfb2e1494566",
    "ruby-3.4.12.x86_64_darwin.tar.gz": "4927c3f886fd89e37bcec36cc7f9f4b0367c6f81381912a0327aafa916971778",
    "ruby-3.4.12.x86_64_linux.tar.gz": "885185fabbc01bf695550d7375e610ae85084f5340474c837e5d00ec6c6cc234",
}
