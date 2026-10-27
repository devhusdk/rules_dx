# frozen_string_literal: true

# Parses Ruby sources with YARD and writes their public API as versioned JSON.
#
# The script never runs the parsed code, YARD's command line or any YARD plugin.
# YARD reads ARGV, ./.yardopts and ~/.yard/config while it is required, so the
# arguments, the working directory and HOME are all neutralized before that.

FORMAT = 1
USAGE = "usage: yard_dump.rb --own FILE --srcs FILE --deps FILE --scratch DIR --out FILE"

def fail_with(message)
  warn "yard_dump: #{message}"
  exit 1
end

def parse_args(argv)
  options = {}
  flags = %w[--own --srcs --deps --scratch --out]
  until argv.empty?
    flag = argv.shift
    fail_with("unknown argument '#{flag}'\n#{USAGE}") unless flags.include?(flag)
    value = argv.shift
    fail_with("#{flag} needs a value\n#{USAGE}") if value.nil? || value.empty?
    fail_with("#{flag} given more than once") if options.key?(flag)
    options[flag] = value
  end
  missing = flags.reject { |flag| options.key?(flag) }
  fail_with("#{missing.join(", ")} is required\n#{USAGE}") unless missing.empty?
  options
end

# Reads one manifest of tab-separated lines; returns the rows as arrays.
def read_rows(path, columns)
  fail_with("cannot read manifest #{path}") unless File.file?(path)
  File.binread(path).force_encoding("UTF-8").split("\n").map do |line|
    row = line.split("\t", -1)
    fail_with("#{path} has a malformed line: #{line.inspect}") unless row.length == columns
    row
  end
end

options = parse_args(ARGV.dup)
ARGV.clear
own_rows = read_rows(options["--own"], 3)
context_rows = read_rows(options["--srcs"], 3)
dep_rows = read_rows(options["--deps"], 2)
fail_with("--own lists no source file") if own_rows.empty?

out = File.expand_path(options["--out"])
scratch = File.expand_path(options["--scratch"])
root = Dir.pwd
absolute = ->(path) { File.expand_path(path, root) }
files = {}
own_rows.each { |exec, rel, generated| files[absolute.call(exec)] = {rel: rel, generated: generated == "1", owner: ""} }
context_rows.each { |exec, rel, generated| files[absolute.call(exec)] ||= {rel: rel, generated: generated == "1", owner: nil} }
dep_rows.each do |package, exec|
  entry = files[absolute.call(exec)]
  fail_with("--deps names #{exec}, which --srcs does not list") if entry.nil?
  entry[:owner] = package if entry[:owner].nil?
end
files.each_key { |path| fail_with("source file #{path} does not exist") unless File.file?(path) }

Dir.mkdir(scratch) unless File.directory?(scratch)
ENV["HOME"] = scratch
ENV["USERPROFILE"] = scratch
ENV.delete("YARD_CONFIG")
Dir.chdir(scratch)
Encoding.default_external = Encoding::UTF_8
Encoding.default_internal = nil

require "json"
begin
  require "yard"
rescue LoadError => e
  fail_with("cannot load YARD: #{e.message}")
end

problems = []
YARD::Logger.prepend(Module.new do
  define_method(:log) do |severity, message|
    problems << message.to_s if severity >= YARD::Logger::Severity::WARN
    super(severity, message)
  end
end)
log.io = $stderr
log.show_backtraces = false
plugins = YARD::Config.options[:autoload_plugins] + YARD::Config.options[:load_plugins].then { |on| on ? ["gems"] : [] }
fail_with("YARD config enables plugins: #{plugins.join(", ")}") unless plugins.empty?

YARD::Registry.clear
# Context and dependency files go first so that a namespace another library
# defines keeps that library as its primary file.
order = files.keys.sort_by { |path| [(files[path][:owner] == "") ? 1 : 0, files[path][:rel]] }
YARD.parse(order, [], YARD::Logger::Severity::WARN)
fail_with("YARD reported #{problems.length} problem(s); no API JSON was written") unless problems.empty?

def type_names(tag)
  tag.types || []
end

def tag_json(tag)
  {
    "tag" => tag.tag_name,
    "name" => tag.name.to_s,
    "types" => type_names(tag),
    "text" => tag.text.to_s
  }
end

def mixin_paths(object, scope)
  object.mixins(scope).map(&:path)
end

def attribute_of(object)
  return nil unless object.type == :method && object.is_attribute?
  name = object.name.to_s.delete_suffix("=")
  pair = object.namespace.attributes[object.scope][name.to_sym]
  {"name" => name, "read" => !pair[:read].nil?, "write" => !pair[:write].nil?}
end

objects = []
YARD::Registry.all(:module, :class, :method, :constant).each do |object|
  entry = files[File.expand_path(object.file.to_s, scratch)]
  next if entry.nil? || entry[:owner].nil?
  namespace = object.namespace
  record = {
    "path" => object.path,
    "type" => object.type.to_s,
    "name" => object.name.to_s,
    "namespace" => namespace.root? ? "" : namespace.path,
    "scope" => object.respond_to?(:scope) ? object.scope.to_s : "",
    "visibility" => object.visibility.to_s,
    "signature" => "",
    "docstring" => object.docstring.to_s,
    "tags" => object.tags.map { |tag| tag_json(tag) },
    "file" => entry[:rel],
    "generated" => entry[:generated],
    "line" => object.line || 0,
    "owner" => entry[:owner],
    "superclass" => "",
    "includes" => [],
    "extends" => [],
    "parameters" => [],
    "attribute" => nil,
    "value" => ""
  }
  case object.type
  when :class
    superclass = object.superclass
    record["superclass"] = superclass.path if superclass && superclass.path != "Object"
    record["includes"] = mixin_paths(object, :instance)
    record["extends"] = mixin_paths(object, :class)
  when :module
    record["includes"] = mixin_paths(object, :instance)
    record["extends"] = mixin_paths(object, :class)
  when :method
    record["signature"] = object.signature.to_s
    record["parameters"] = object.parameters.map { |name, _default| name.to_s }
    record["attribute"] = attribute_of(object)
  when :constant
    record["value"] = object.value.to_s
  end
  objects << record
end
objects.sort_by! { |record| [record["path"], record["scope"]] }
fail_with("the library's own files define no Ruby object") if objects.none? { |record| record["owner"] == "" }

document = {
  "format" => FORMAT,
  "ruby" => RUBY_VERSION,
  "yard" => YARD::VERSION,
  "objects" => objects
}
File.open(out, "wb") { |file| file.write(JSON.pretty_generate(document) + "\n") }
