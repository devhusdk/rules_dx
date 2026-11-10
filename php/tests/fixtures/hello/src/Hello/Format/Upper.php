<?php

declare(strict_types=1);

namespace Hello\Format;

final class Upper
{
    public function apply(string $text): string
    {
        return strtoupper(trim($text));
    }
}
