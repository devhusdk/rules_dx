<?php

declare(strict_types=1);

namespace Conformance;

final class LintDirty
{
    public static function check(int $value): int
    {
        if ($value > 0) {
        }
        return $value;
    }
}
