<?php

declare(strict_types=1);

final class Lib
{
    public function used(): int
    {
        return 1;
    }

    public function uncalled(): int
    {
        return 2;
    }
}

echo (new Lib())->used();
