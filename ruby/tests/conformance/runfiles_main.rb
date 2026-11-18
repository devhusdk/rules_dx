# frozen_string_literal: true

require "bazel/runfiles"

$stdout.binmode
$stdout.write(File.binread(Bazel::Runfiles.create.rlocation(ARGV.fetch(0))))
