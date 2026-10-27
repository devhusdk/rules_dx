# Prints the sorted list of features shipped with the running Ruby, one per
# line. With --check <file>, diffs that list against <file> instead and exits
# non-zero on drift. Bundled gems are excluded: they need a Gemfile entry.
require "rbconfig"

def features
  set = {}
  [RbConfig::CONFIG["rubylibdir"], RbConfig::CONFIG["rubyarchdir"]].compact.uniq.each do |dir|
    Dir.glob("**/*.{rb,so}", base: dir).each do |f|
      next if f.start_with?("#{RbConfig::CONFIG["arch"]}/")
      set[f.sub(/\.(rb|so)\z/, "")] = true
    end
  end
  $LOADED_FEATURES.each do |f|
    next if f.start_with?("/")
    set[f.sub(/\.(rb|so)\z/, "")] = true
  end
  set.keys.sort
end

if ARGV[0] == "--check"
  want = features
  have = File.read(ARGV[1]).split("\n").reject(&:empty?)
  missing = want - have
  extra = have - want
  exit 0 if missing.empty? && extra.empty?
  warn "stdlib.txt drifted from Ruby #{RUBY_VERSION}; regenerate with: bazel run //gazelle/ruby:stdlib_dump > gazelle/ruby/stdlib.txt"
  missing.each { |f| warn "missing: #{f}" }
  extra.each { |f| warn "extra: #{f}" }
  if (out = ENV["TEST_UNDECLARED_OUTPUTS_DIR"])
    File.write(File.join(out, "stdlib.txt"), want.join("\n") + "\n")
  end
  exit 1
else
  puts features
end
