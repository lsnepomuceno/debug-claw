<?php

// The container mounts vendor/ at /opt/vendor, as images that install
// Composer packages outside the bind mount do.
$vendor = is_dir('/opt/vendor/acme') ? '/opt/vendor' : __DIR__ . '/vendor';
require $vendor . '/acme/lib/src/Lib.php';

$total = 0;
for ($i = 1; $i <= 2; $i++) {
    $total += Acme\Lib\Lib::double($i);
}
echo $total, "\n";
