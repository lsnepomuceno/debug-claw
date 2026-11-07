<?php

declare(strict_types=1);

// Class constants, statics and an enum, for the property tree probe.
enum Status: string
{
    case Active = 'active';
    case Closed = 'closed';
}

class Base
{
    public const VERSION = '1.0';
    protected static int $instances = 0;
}

final class Account extends Base
{
    public const LIMIT = 100;
    private const SECRET = 'hidden';
    public static string $currency = 'EUR';
    private static array $registry = ['a' => 1];

    public function __construct(
        public readonly int $id,
        public Status $status = Status::Active,
        private float $balance = 10.5,
    ) {
        self::$instances++;
        self::$registry[$id] = $id;
    }

    public function balance(): float
    {
        $copy = $this->balance;

        return $copy;
    }
}

const APP_MODE = 'probe';
define('APP_DEBUG', true);

$account = new Account(7);
$balance = $account->balance();
echo $balance, "\n";
