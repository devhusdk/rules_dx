<?php

declare(strict_types=1);

use PhpBench\Attributes\Iterations;
use PhpBench\Attributes\Revs;

final class SumBench
{
    #[Revs(50), Iterations(3)]
    public function benchSumLoop(): int
    {
        $total = 0;
        for ($i = 1; $i <= 1000; $i++) {
            $total += $i;
        }

        return $total;
    }

    #[Revs(50), Iterations(3)]
    public function benchArraySum(): int
    {
        return array_sum(range(1, 1000));
    }
}
