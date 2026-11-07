#!/bin/sh
# Prints the CPU set of every running container of the test bed.
set -eu
. "$(dirname "$0")/lib.sh"
for id in $(docker ps -q --filter "label=com.docker.compose.project=$COMPOSE_PROJECT_NAME"); do
    docker inspect -f '{{.Name}} {{.HostConfig.CpusetCpus}}' "$id"
done
