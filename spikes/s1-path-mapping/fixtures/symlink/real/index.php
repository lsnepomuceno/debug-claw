<?php

// Run as /srv/sym/current/index.php, where current -> real.
require __DIR__ . '/lib.php';
require '/srv/sym/current/other.php';

echo greet('world'), ' ', other(), "\n";
