<?php
declare(strict_types=1);

function fail(string $message): never
{
    fwrite(STDERR, "php_dump: $message\n");
    exit(1);
}

function parse_args(array $argv): array
{
    $arg = [];
    for ($i = 1; $i < count($argv); $i += 2) {
        $flag = $argv[$i];
        if (!in_array($flag, ['--own', '--out'], true)) {
            fail("unknown argument '$flag'");
        }
        if (!isset($argv[$i + 1])) {
            fail("$flag needs a value");
        }
        $arg[substr($flag, 2)] = $argv[$i + 1];
    }
    if (!isset($arg['own'], $arg['out'])) {
        fail('--own and --out are required');
    }
    return $arg;
}

function clean_doc(string $comment): string
{
    $comment = str_replace("\r\n", "\n", $comment);
    $comment = preg_replace('~^/\*\*+~', '', $comment);
    $comment = preg_replace('~\*+/$~', '', $comment);
    $lines = [];
    foreach (explode("\n", $comment) as $line) {
        $lines[] = preg_replace('~^\s*\*\s?~', '', rtrim($line));
    }
    return trim(implode("\n", $lines));
}

function significant(array $tokens, int $from): int
{
    for ($i = $from; $i < count($tokens); $i++) {
        $t = $tokens[$i];
        if (is_array($t) && in_array($t[0], [T_WHITESPACE, T_COMMENT, T_DOC_COMMENT], true)) {
            continue;
        }
        return $i;
    }
    return -1;
}

function text(mixed $token): string
{
    return is_array($token) ? $token[1] : $token;
}

function signature(array $tokens, int $from, array $modifiers): string
{
    $out = '';
    $paren = 0;
    for ($i = $from; $i < count($tokens); $i++) {
        $t = $tokens[$i];
        $s = text($t);
        if ($s === '(') {
            $paren++;
        } elseif ($s === ')') {
            $paren--;
        } elseif ($paren === 0 && ($s === '{' || $s === ';')) {
            break;
        }
        if (is_array($t) && in_array($t[0], [T_COMMENT, T_DOC_COMMENT], true)) {
            continue;
        }
        $out .= $s;
    }
    $out = trim(preg_replace('~\s+~', ' ', $out));
    return trim(implode(' ', $modifiers) . ' ' . $out);
}

function dump_file(string $path, string $repo, bool $generated, array &$objects): void
{
    $source = file_get_contents($path);
    if ($source === false) {
        fail("cannot read $path");
    }
    $source = str_replace("\r\n", "\n", $source);
    $tokens = token_get_all($source, TOKEN_PARSE);
    $namespace = '';
    $depth = 0;
    $base = 0;
    $classes = [];
    $pending = null;
    $doc = '';
    $modifiers = [];
    $record = static function (string $type, string $path, string $name, string $owner, string $sig, string $doc, int $line, string $visibility) use (&$objects, $repo, $generated): void {
        $objects[] = [
            'type' => $type,
            'path' => $path,
            'name' => $name,
            'namespace' => $owner,
            'signature' => $sig,
            'docstring' => $doc,
            'file' => $repo,
            'generated' => $generated,
            'line' => $line,
            'visibility' => $visibility,
        ];
    };
    $qualify = static function (string $name) use (&$namespace): string {
        return $namespace === '' ? $name : $namespace . '\\' . $name;
    };
    $count = count($tokens);
    for ($i = 0; $i < $count; $i++) {
        $t = $tokens[$i];
        $id = is_array($t) ? $t[0] : null;
        $s = text($t);
        $line = is_array($t) ? $t[2] : 0;
        if ($id === T_DOC_COMMENT) {
            $doc = clean_doc($s);
            continue;
        }
        if ($id === T_WHITESPACE || $id === T_COMMENT) {
            continue;
        }
        if ($id === T_NAMESPACE) {
            $n = significant($tokens, $i + 1);
            if ($n >= 0 && is_array($tokens[$n]) && in_array($tokens[$n][0], [T_STRING, T_NAME_QUALIFIED], true)) {
                $namespace = $tokens[$n][1];
                $m = significant($tokens, $n + 1);
                if ($m >= 0 && $tokens[$m] === '{') {
                    $base = $depth + 1;
                }
            } elseif ($n >= 0 && $tokens[$n] === '{') {
                $namespace = '';
                $base = $depth + 1;
            }
            $doc = '';
            continue;
        }
        if (in_array($id, [T_PUBLIC, T_PROTECTED, T_PRIVATE, T_STATIC, T_ABSTRACT, T_FINAL, T_READONLY, T_VAR], true)) {
            $modifiers[] = $s;
            continue;
        }
        if (in_array($id, [T_CLASS, T_INTERFACE, T_TRAIT, T_ENUM], true)) {
            $prev = $i - 1;
            while ($prev >= 0 && is_array($tokens[$prev]) && in_array($tokens[$prev][0], [T_WHITESPACE, T_COMMENT, T_DOC_COMMENT], true)) {
                $prev--;
            }
            $before = $prev >= 0 ? $tokens[$prev] : null;
            $n = significant($tokens, $i + 1);
            $named = $n >= 0 && is_array($tokens[$n]) && $tokens[$n][0] === T_STRING;
            if ($id === T_CLASS && is_array($before) && $before[0] === T_NEW) {
                $pending = ['name' => null];
            } elseif ($named && !(is_array($before) && $before[0] === T_DOUBLE_COLON)) {
                $kind = strtolower($s);
                $name = $tokens[$n][1];
                $top = end($classes);
                $top_level = $top === false && $depth <= $base;
                if ($top_level) {
                    $path = $qualify($name);
                    $record($kind, $path, $name, '', signature($tokens, $i, $modifiers), $doc, $line, 'public');
                    $pending = ['name' => $path];
                } else {
                    $pending = ['name' => null];
                }
            }
            $doc = '';
            $modifiers = [];
            continue;
        }
        if ($id === T_FUNCTION) {
            $n = significant($tokens, $i + 1);
            if ($n >= 0 && $tokens[$n] === '&') {
                $n = significant($tokens, $n + 1);
            }
            if ($n >= 0 && is_array($tokens[$n]) && $tokens[$n][0] === T_STRING) {
                $name = $tokens[$n][1];
                $top = end($classes);
                if ($top !== false && $top['name'] !== null && $depth === $top['depth'] + 1) {
                    $private = in_array('private', array_map('strtolower', $modifiers), true) || in_array('protected', array_map('strtolower', $modifiers), true);
                    $record('method', $top['name'] . '::' . $name, $name, $top['name'], signature($tokens, $i, $modifiers), $doc, $line, $private ? 'private' : 'public');
                } elseif ($top === false && $depth <= $base) {
                    $record('function', $qualify($name), $name, '', signature($tokens, $i, $modifiers), $doc, $line, 'public');
                }
            }
            $doc = '';
            $modifiers = [];
            continue;
        }
        if ($s === '{' || $id === T_CURLY_OPEN || $id === T_DOLLAR_OPEN_CURLY_BRACES) {
            $depth++;
            if ($s === '{' && $pending !== null) {
                $classes[] = ['name' => $pending['name'], 'depth' => $depth - 1];
                $pending = null;
            }
            $doc = '';
            $modifiers = [];
            continue;
        }
        if ($s === '}') {
            $depth--;
            $top = end($classes);
            if ($top !== false && $top['depth'] === $depth) {
                array_pop($classes);
            }
            $doc = '';
            $modifiers = [];
            continue;
        }
        if ($s === ';') {
            $doc = '';
            $modifiers = [];
        }
    }
}

$arg = parse_args($argv);
$list = file($arg['own'], FILE_IGNORE_NEW_LINES);
if ($list === false) {
    fail("cannot read {$arg['own']}");
}
$objects = [];
foreach ($list as $row) {
    if (trim($row) === '') {
        continue;
    }
    [$path, $repo, $generated] = explode("\t", $row, 3);
    dump_file($path, $repo, $generated === '1', $objects);
}
$json = json_encode(
    ['format' => 1, 'php' => PHP_VERSION, 'objects' => $objects],
    JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR,
);
if (file_put_contents($arg['out'], $json . "\n") === false) {
    fail("cannot write {$arg['out']}");
}
