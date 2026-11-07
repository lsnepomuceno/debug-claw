<?php

// Runs for about 5 seconds, for the asynchronous break probe.
$deadline = microtime(true) + 5;
$spins = 0;
while (microtime(true) < $deadline) {
    $spins++;
}
echo $spins, "\n";
