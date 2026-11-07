<?php

// T-11: the fixture app has no private, protected or static properties and
// no constants, so this CLI script adds them. Run with the trigger set:
//   XDEBUG_TRIGGER=1 php /probe/visibility.php

declare(strict_types=1);

const APP_MODE = 'probe';
define('APP_LEVEL', 3);

class Base
{
    public const string KIND = 'base';
    protected static int $instances = 0;
    private string $baseSecret = 'base-private';
    protected array $tags = ['x' => 1, 7 => 'seven'];
}

final class Account extends Base
{
    public static string $currency = 'EUR';
    private static array $registry = ['a' => 1];
    private string $secret = 'account-private';
    protected float $balance = 10.5;
    public ?Account $next = null;
    public array $nested = ['level1' => ['level2' => [0 => 'deep', 'k' => 'v']]];

    public function balance(): float
    {
        self::$instances++;
        $local = "balance is {$this->balance}";
        return $this->balance; // line 33: breakpoint for T-11
    }
}

$account = new Account();
$account->next = new Account();
$list = [10, 20, 'key' => ['inner' => 30]];
echo $account->balance(), "\n";
