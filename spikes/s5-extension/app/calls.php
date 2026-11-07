<?php

// Cost of the extension for functions it does not observe, from the CLI.
//
//   php /s5/app/calls.php [CALLS=2000000] [REPS=9]
//
// Calls a user function, a user method, a closure and two internal
// functions CALLS times each, REPS times, and prints the median nanoseconds
// per call. Run it with and without the extension: the observer's init
// callback returns no handler for these, so PHP should not call into the
// extension after the first call of each function.

declare(strict_types=1);

$calls = (int) ($argv[1] ?? 2000000);
$reps = (int) ($argv[2] ?? 9);

function plusOne(int $x): int
{
    return $x + 1;
}

final class Counter
{
    public function next(int $x): int
    {
        return $x + 1;
    }
}

$counter = new Counter();
$closure = static fn (int $x): int => $x + 1;
$text = 'debug-claw';

$cases = [
    'user_function' => static function (int $n): void {
        for ($i = 0; $i < $n; $i++) {
            plusOne($i);
        }
    },
    'user_method' => static function (int $n) use ($counter): void {
        for ($i = 0; $i < $n; $i++) {
            $counter->next($i);
        }
    },
    'closure' => static function (int $n) use ($closure): void {
        for ($i = 0; $i < $n; $i++) {
            $closure($i);
        }
    },
    'internal_str_repeat' => static function (int $n) use ($text): void {
        for ($i = 0; $i < $n; $i++) {
            str_repeat($text, 2);
        }
    },
    'internal_max' => static function (int $n): void {
        for ($i = 0; $i < $n; $i++) {
            max($i, 7);
        }
    },
];

$out = [];
foreach ($cases as $name => $run) {
    $run(10000);
    $perCall = [];
    for ($r = 0; $r < $reps; $r++) {
        $t = hrtime(true);
        $run($calls);
        $perCall[] = (hrtime(true) - $t) / $calls;
    }
    sort($perCall);
    $out[] = sprintf('%s=%.2f[%.2f-%.2f]', $name, $perCall[intdiv($reps, 2)], $perCall[0], $perCall[$reps - 1]);
}

printf(
    "calls n=%d reps=%d ns_per_call %s ext=%s\n",
    $calls,
    $reps,
    implode(' ', $out),
    function_exists('debug_claw_s5_stats') ? debug_claw_s5_stats() : 'not-loaded',
);
