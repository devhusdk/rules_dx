<?php

declare(strict_types=1);

// Reads PHP sources with the pinned phpDocumentor and writes their API as versioned JSON.
//
// Usage: php_dump.php --phar FILE --tool-version V --own FILE --scratch DIR --out FILE

use phpDocumentor\Reflection\DocBlock;
use phpDocumentor\Reflection\DocBlock\Tags\BaseTag;
use phpDocumentor\Reflection\File\LocalFile;
use phpDocumentor\Reflection\Php\ProjectFactory;

error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);

const FORMAT = 1;
const USAGE = 'usage: php_dump.php --phar FILE --tool-version V --own FILE --scratch DIR --out FILE';

function fail_with(string $message): never
{
    fwrite(STDERR, 'php_dump: ' . $message . "\n");
    exit(1);
}

/** @return array<string, string> */
function parse_args(array $argv): array
{
    $flags = ['--phar', '--tool-version', '--own', '--scratch', '--out'];
    $options = [];
    $args = array_slice($argv, 1);
    while ($args !== []) {
        $flag = array_shift($args);
        if (!in_array($flag, $flags, true)) {
            fail_with("unknown argument '$flag'\n" . USAGE);
        }
        $value = array_shift($args);
        if ($value === null || $value === '') {
            fail_with("$flag needs a value\n" . USAGE);
        }
        if (array_key_exists($flag, $options)) {
            fail_with("$flag given more than once");
        }
        $options[$flag] = $value;
    }
    $missing = array_diff($flags, array_keys($options));
    if ($missing !== []) {
        fail_with(implode(', ', $missing) . " is required\n" . USAGE);
    }
    return $options;
}

/** @return list<array{0: string, 1: string, 2: bool}> */
function read_rows(string $path): array
{
    $text = @file_get_contents($path);
    if ($text === false) {
        fail_with("cannot read manifest $path");
    }
    $rows = [];
    foreach (explode("\n", $text) as $line) {
        if ($line === '') {
            continue;
        }
        $row = explode("\t", $line);
        if (count($row) !== 3) {
            fail_with("$path has a malformed line: " . json_encode($line));
        }
        $rows[] = [$row[0], $row[1], $row[2] === '1'];
    }
    return $rows;
}

function text(mixed $value): string
{
    return $value === null ? '' : trim((string) $value);
}

/** @return array{summary: string, description: string, tags: list<array<string, mixed>>} */
function doc(?DocBlock $block): array
{
    $out = ['summary' => '', 'description' => '', 'tags' => []];
    if ($block === null) {
        return $out;
    }
    $out['summary'] = text($block->getSummary());
    $out['description'] = text($block->getDescription()->render());
    foreach ($block->getTags() as $tag) {
        $entry = ['tag' => $tag->getName(), 'name' => '', 'type' => '', 'text' => ''];
        if (method_exists($tag, 'getVariableName')) {
            $entry['name'] = text($tag->getVariableName());
        }
        if (method_exists($tag, 'getType')) {
            $entry['type'] = text($tag->getType());
        }
        if ($tag instanceof BaseTag) {
            $entry['text'] = text($tag->getDescription()?->render());
        } elseif (method_exists($tag, 'getDescription')) {
            $entry['text'] = text($tag->getDescription()?->render());
        } else {
            $entry['text'] = text((string) $tag);
        }
        $out['tags'][] = $entry;
    }
    return $out;
}

/** @return list<array<string, mixed>> */
function arguments(object $method): array
{
    $out = [];
    foreach ($method->getArguments() as $argument) {
        $out[] = [
            'name' => $argument->getName(),
            'type' => text($argument->getType()),
            'default' => text($argument->getDefault(false)),
            'variadic' => $argument->isVariadic(),
            'by_ref' => $argument->isByReference(),
        ];
    }
    return $out;
}

/** @param list<object> $items */
function names(iterable $items): array
{
    $out = [];
    foreach ($items as $item) {
        $out[] = ltrim((string) $item, '\\');
    }
    sort($out);
    return $out;
}

$options = parse_args($argv);
$rows = read_rows($options['--own']);
if ($rows === []) {
    fail_with('--own lists no source file');
}
$scratch = $options['--scratch'];
if (!is_dir($scratch) && !mkdir($scratch, 0777, true)) {
    fail_with("cannot create $scratch");
}
$scratch = realpath($scratch);
$out = $options['--out'];
$root = getcwd();
putenv('HOME=' . $scratch);
chdir($scratch);

require 'phar://' . (str_starts_with($options['--phar'], '/') ? $options['--phar'] : $root . '/' . $options['--phar']) . '/vendor/autoload.php';

$rel = [];
$generated = [];
$files = [];
foreach ($rows as [$exec, $relative, $is_generated]) {
    $absolute = $exec[0] === '/' ? $exec : $root . '/' . $exec;
    if (!is_file($absolute)) {
        fail_with("source file $absolute does not exist");
    }
    $rel[$absolute] = $relative;
    $generated[$absolute] = $is_generated;
    $files[] = new LocalFile($absolute);
}

try {
    $project = ProjectFactory::createInstance()->create('dx', $files);
} catch (Throwable $error) {
    fail_with('cannot read the sources: ' . $error->getMessage());
}

$objects = [];

function base(string $type, object $element, string $parent, string $file, array $rel, array $generated): array
{
    $doc = doc($element->getDocBlock());
    $line = $element->getLocation()->getLineNumber();
    return [
        'type' => $type,
        'fqsen' => ltrim((string) $element->getFqsen(), '\\'),
        'name' => $element->getName(),
        'parent' => $parent,
        'file' => $rel[$file],
        'generated' => $generated[$file],
        'line' => $line,
        'summary' => $doc['summary'],
        'description' => $doc['description'],
        'tags' => $doc['tags'],
        'visibility' => 'public',
        'abstract' => false,
        'final' => false,
        'static' => false,
        'readonly' => false,
        'superclass' => '',
        'interfaces' => [],
        'traits' => [],
        'params' => [],
        'return_type' => '',
        'value_type' => '',
        'value' => '',
    ];
}

function members(array &$objects, object $owner, string $parent, string $file, array $rel, array $generated): void
{
    foreach ($owner->getConstants() as $constant) {
        $o = base('constant', $constant, $parent, $file, $rel, $generated);
        $o['visibility'] = (string) $constant->getVisibility();
        $o['final'] = $constant->isFinal();
        $o['value'] = text($constant->getValue());
        $objects[] = $o;
    }
    if (method_exists($owner, 'getProperties')) {
        foreach ($owner->getProperties() as $property) {
            $o = base('property', $property, $parent, $file, $rel, $generated);
            $o['visibility'] = (string) $property->getVisibility();
            $o['static'] = $property->isStatic();
            $o['readonly'] = $property->isReadOnly();
            $o['value_type'] = text($property->getType());
            $o['value'] = text($property->getDefault(false));
            $objects[] = $o;
        }
    }
    foreach ($owner->getMethods() as $method) {
        $o = base('method', $method, $parent, $file, $rel, $generated);
        $o['visibility'] = (string) $method->getVisibility();
        $o['abstract'] = $method->isAbstract();
        $o['final'] = $method->isFinal();
        $o['static'] = $method->isStatic();
        $o['params'] = arguments($method);
        $o['return_type'] = text($method->getReturnType());
        $objects[] = $o;
    }
}

foreach ($project->getFiles() as $file) {
    $path = $file->getPath();
    if (!isset($rel[$path])) {
        fail_with("phpDocumentor reports the unexpected file $path");
    }
    $namespace = static fn (object $e): string => ltrim((string) $e->getFqsen(), '\\');
    foreach ($file->getClasses() as $class) {
        $o = base('class', $class, '', $path, $rel, $generated);
        $o['abstract'] = $class->isAbstract();
        $o['final'] = $class->isFinal();
        $o['readonly'] = $class->isReadOnly();
        $o['superclass'] = $class->getParent() === null ? '' : ltrim((string) $class->getParent(), '\\');
        $o['interfaces'] = names($class->getInterfaces());
        $o['traits'] = names($class->getUsedTraits());
        $objects[] = $o;
        members($objects, $class, $o['fqsen'], $path, $rel, $generated);
    }
    foreach ($file->getInterfaces() as $interface) {
        $o = base('interface', $interface, '', $path, $rel, $generated);
        $o['interfaces'] = names($interface->getParents());
        $objects[] = $o;
        members($objects, $interface, $o['fqsen'], $path, $rel, $generated);
    }
    foreach ($file->getTraits() as $trait) {
        $o = base('trait', $trait, '', $path, $rel, $generated);
        $o['traits'] = names($trait->getUsedTraits());
        $objects[] = $o;
        members($objects, $trait, $o['fqsen'], $path, $rel, $generated);
    }
    foreach ($file->getEnums() as $enum) {
        $o = base('enum', $enum, '', $path, $rel, $generated);
        $o['value_type'] = text($enum->getBackedType());
        $o['interfaces'] = names($enum->getInterfaces());
        $o['traits'] = names($enum->getUsedTraits());
        $objects[] = $o;
        foreach ($enum->getCases() as $case) {
            $c = base('case', $case, $o['fqsen'], $path, $rel, $generated);
            $c['value'] = text($case->getValue(false));
            $objects[] = $c;
        }
        members($objects, $enum, $o['fqsen'], $path, $rel, $generated);
    }
    foreach ($file->getFunctions() as $function) {
        $o = base('function', $function, '', $path, $rel, $generated);
        $o['params'] = arguments($function);
        $o['return_type'] = text($function->getReturnType());
        $objects[] = $o;
    }
    foreach ($file->getConstants() as $constant) {
        $o = base('constant', $constant, '', $path, $rel, $generated);
        $o['value'] = text($constant->getValue());
        $objects[] = $o;
    }
}

$json = json_encode(
    [
        'format' => FORMAT,
        'php' => PHP_VERSION,
        'phpdocumentor' => $options['--tool-version'],
        'objects' => $objects,
    ],
    JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR | JSON_PRETTY_PRINT,
);
if (file_put_contents($out[0] === '/' ? $out : $root . '/' . $out, $json . "\n") === false) {
    fail_with("cannot write $out");
}
