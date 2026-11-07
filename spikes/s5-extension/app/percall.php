<?php

// Per-call cost of the observer on PDOStatement::execute, from the CLI.
//
//   php /s5/app/percall.php [CALLS=20000] [REPS=9]
//
// Times CALLS executes of a prepared "SELECT ?" on in-memory SQLite, REPS
// times, and prints the median and the spread in nanoseconds per call. The
// CLI has one request, so with debug_claw.flush=request the events stay in
// the buffer until it is full and are then dropped; with flush=call every
// execute writes (or, with the daemon down, skips) at once.

declare(strict_types=1);

$calls = (int) ($argv[1] ?? 20000);
$reps = (int) ($argv[2] ?? 9);

$pdo = new PDO('sqlite::memory:');
$pdo->setAttribute(PDO::ATTR_ERRMODE, PDO::ERRMODE_EXCEPTION);
$st = $pdo->prepare('SELECT ?');

for ($i = 0; $i < 2000; $i++) {
    $st->execute([$i]);
}

$perCall = [];
for ($r = 0; $r < $reps; $r++) {
    $t = hrtime(true);
    for ($i = 0; $i < $calls; $i++) {
        $st->execute([$i]);
    }
    $perCall[] = (hrtime(true) - $t) / $calls;
}
sort($perCall);

printf(
    "percall calls=%d reps=%d median_ns=%.0f min_ns=%.0f max_ns=%.0f ext=%s\n",
    $calls,
    $reps,
    $perCall[intdiv($reps, 2)],
    $perCall[0],
    $perCall[$reps - 1],
    function_exists('debug_claw_s5_stats') ? debug_claw_s5_stats() : 'not-loaded',
);
