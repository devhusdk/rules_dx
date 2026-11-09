<?php

declare(strict_types=1);

namespace Dependent;

use Clean\Answer;

final class Check
{
    public static function value(): int
    {
        return Answer::value();
    }
}
