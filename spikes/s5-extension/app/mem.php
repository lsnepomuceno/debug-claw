<?php

// Reports the worker's memory as PHP sees it, and the extension's counters.
// Called with cgi-fcgi from inside the PHP container, so it lands on the
// single FPM worker without going through nginx.

declare(strict_types=1);

$status = [];
foreach (file('/proc/self/status') as $line) {
    if (preg_match('/^(VmRSS|VmHWM|RssAnon|RssFile):\s+(\d+) kB/', $line, $m)) {
        $status[$m[1] . '_kb'] = (int) $m[2];
    }
}

header('Content-Type: application/json');
echo json_encode([
    'pid' => getmypid(),
    'memory_get_usage' => memory_get_usage(),
    'memory_get_usage_real' => memory_get_usage(true),
    'memory_get_peak_usage' => memory_get_peak_usage(),
    'proc' => $status,
    'gc' => gc_status()['runs'],
    'ext' => function_exists('debug_claw_s5_stats') ? json_decode(debug_claw_s5_stats(), true) : null,
]), "\n";
