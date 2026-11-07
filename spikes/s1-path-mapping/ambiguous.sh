#!/bin/sh
# T-05: the cases where inference fails or is ambiguous. Each run writes
# a transcript to results/.
set -eu
cd "$(dirname "$0")"
F=/work/spikes/s1-path-mapping/fixtures
php85='dc exec -T -e XDEBUG_TRIGGER=1 php85 php'

# Same relative path in two places, different contents.
./run.sh case-dup.txt "map --local $F/dup --break admin/public/index.php:4" \
    "$php85 /var/www/admin/public/index.php"

# Same relative path in two places, identical contents.
./run.sh case-dup-identical.txt "map --local $F/dup-identical --break b/public/index.php:4" \
    "$php85 /var/www/same/public/index.php"

# The local root holds only the other copy: unique by name, other content.
./run.sh case-stale.txt "map --local $F/dup/api --break public/index.php:4" \
    "$php85 /var/www/admin/public/index.php"

# Vendor mounted elsewhere: the translated vendor path does not exist.
./run.sh case-vendor-missing.txt "map --local $F/vendored --break vendor/acme/lib/src/Lib.php:9" \
    "$php85 /srv/vend/index.php"

# Vendor mounted elsewhere: learn the second mapping from a stack frame.
./run.sh case-vendor-learned.txt "vendor --local $F/vendored --app-break index.php:10 --vendor-break vendor/acme/lib/src/Lib.php:9" \
    "$php85 /srv/vend/index.php"

# Symlinked folder (current -> real), breakpoints through the link.
./run.sh case-symlink-link.txt "map --local $F/symlink --break current/lib.php:5 --break current/other.php:5" \
    "$php85 /srv/sym/current/index.php"

# Same, breakpoints on the real folder.
./run.sh case-symlink-real.txt "map --local $F/symlink --break real/lib.php:5 --break real/other.php:5" \
    "$php85 /srv/sym/current/index.php"

# A breakpoint whose case differs from the file on disk (Linux).
./run.sh case-wrong-case.txt "map --local /work/spikes/testbed/app --break src/graph.php:17" \
    "curl -s -b XDEBUG_TRIGGER=1 'http://127.0.0.1:8180/graph?depth=2&width=1'"
