<?php

namespace Acme\Lib;

final class Lib
{
    public static function double(int $n): int
    {
        $result = $n * 2;

        return $result;
    }
}
