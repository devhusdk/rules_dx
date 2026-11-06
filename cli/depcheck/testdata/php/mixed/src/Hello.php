<?php
namespace Example;

use Monolog\Logger;
use Psr\Log\LoggerInterface;

class Hello
{
    public function __construct(private LoggerInterface $log = new Logger('hello')) {}
}
