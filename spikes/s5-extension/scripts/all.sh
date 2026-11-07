#!/bin/sh
# Every measurement of the report, one after another (about 10 minutes on
# the test machine). Nothing else should use the pinned CPUs meanwhile.
set -eu
cd "$(dirname "$0")"
[ "${SKIP_PHP85_QUERIES:-}" ] || ./overhead.sh php85 10 2000 200
./overhead.sh php83 10 2000 200
./overhead.sh php85 10 2000 200 / root
./overhead.sh php85 10 2000 200 /graph graph
./overhead.sh php83 10 2000 200 /graph graph
./cli.sh php85 3
./cli.sh php83 3
./soak.sh php85 off
./soak.sh php85 on
./soak.sh php83 off
./soak.sh php83 on
