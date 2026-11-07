<?php

// One of each PHP error kind, then an exception, for exception
// breakpoints on errors.
function notice(): string
{
    return end(explode(',', 'a,b'));
}

$values = [];
$deprecated = strlen(null);
$warning = $values['missing'];
$notice = notice();
try {
    throw new RuntimeException('caught');
} catch (RuntimeException $e) {
}
echo "done\n";
