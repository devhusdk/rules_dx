<?php

namespace Tool;

use ArrayObject;
use function strlen;

// new Not\Real
final class Tool extends ArrayObject implements \JsonSerializable
{
    public function jsonSerialize(): mixed
    {
        $text = <<<TXT
        new Heredoc\Name
        TXT;
        return [strlen($text), PHP_EOL, new \DateTimeImmutable('now'), 'new Str\Name'];
    }
}
