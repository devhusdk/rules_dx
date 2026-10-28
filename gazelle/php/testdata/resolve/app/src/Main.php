<?php

namespace Acme\App;

use Acme\Widgets\Widget;
use Monolog\Logger;

function boot(): void
{
    $widget = new Widget();
    $helper = new Helper();
    $logger = new Logger('app');
    throw new \RuntimeException('unused');
}
