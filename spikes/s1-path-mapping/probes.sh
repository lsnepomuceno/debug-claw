#!/bin/sh
# T-06: probes the DBGp features the design depends on. Each run writes a
# transcript to results/probe-*.txt.
set -eu
cd "$(dirname "$0")"
P=/work/spikes/s1-path-mapping/probes
php85='dc exec -T -e XDEBUG_TRIGGER=1 php85 php'
php83='dc exec -T -e XDEBUG_TRIGGER=1 php83 php'
script="script --max 4000 --cmd-file"

# ./probes.sh ctrl runs only the control socket probes.
if [ "${1:-all}" != ctrl ]; then
./run.sh probe-features-php85.txt "$script $P/features.txt" "$php85 /probe/output.php"
./run.sh probe-features-php83.txt "$script $P/features.txt" "$php83 /probe/output.php"
./run.sh probe-stdout.txt "$script $P/stdout.txt" "$php85 /probe/output.php"
./run.sh probe-stdout-redirect.txt "$script $P/stdout-redirect.txt" "$php85 /probe/output.php"
./run.sh probe-statics.txt "script --max 12000 --cmd-file $P/statics.txt" "$php85 /probe/statics.php"
./run.sh probe-callreturn.txt "$script $P/callreturn.txt" "$php85 /probe/statics.php"
./run.sh probe-exceptions.txt "$script $P/exceptions.txt" "$php85 /probe/errors.php"
./run.sh probe-exceptions-any.txt "$script $P/exceptions-any.txt" "$php85 /probe/errors.php"
./run.sh probe-warning-route.txt "$script $P/warning-route.txt" \
    "curl -s -b XDEBUG_TRIGGER=1 http://127.0.0.1:8180/warning"
./run.sh probe-async-break.txt "$script $P/async-break.txt" "$php85 /probe/long.php"

fi

# Control socket, with a DBGp session open: ps from php85 and from the
# toolchain container, then pause.
./run.sh probe-ctrl-pause.txt "$script $P/ctrl-pause.txt" \
    "$php85 /probe/long.php >/dev/null 2>&1 & sleep 1.5;
     dc exec -T php85 sh /probe/ctrl.sh list;
     name=\$(dc exec -T php85 sh /probe/ctrl.sh list | grep -o 'xdebug-ctrl[.0-9]*');
     echo \"from toolchain to \$name:\"; dc exec -T toolchain /target/debug/s1-path-mapping ctrl \$name ps;
     echo 'from php85:'; dc exec -T php85 sh /probe/ctrl.sh ps; dc exec -T php85 sh /probe/ctrl.sh pause; wait"

# Control socket, no DBGp session (no trigger): pause asks Xdebug to
# connect to the client.
./run.sh probe-ctrl-attach.txt "$script $P/ctrl-attach.txt" \
    "dc exec -T php85 php /probe/long.php >/dev/null 2>&1 & sleep 1.5;
     dc exec -T php85 sh /probe/ctrl.sh list; dc exec -T php85 sh /probe/ctrl.sh pause; wait"
