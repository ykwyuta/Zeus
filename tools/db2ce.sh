#!/bin/sh
# Starts Db2 Community Edition in a container for Zeus (the image is
# downloaded from IBM's registry; it is not part of Zeus).
#
# usage: tools/db2ce.sh start   # prints the ZEUS_DB2 URL when the database is up
#        tools/db2ce.sh stop
#
# The database is ZEUSDB, user db2inst1, on port ${ZEUS_DB2_PORT:-50000}.
set -eu

name=zeus-db2ce
image=icr.io/db2_community/db2:latest
port=${ZEUS_DB2_PORT:-50000}
password=${ZEUS_DB2_PASSWORD:-zeuspass}

case ${1:-} in
    start)
        if ! docker ps --format '{{.Names}}' | grep -qx $name; then
            docker rm -f $name >/dev/null 2>&1 || true
            docker run -d --name $name --privileged=true -p "$port:50000" \
                -e LICENSE=accept -e DB2INSTANCE=db2inst1 \
                -e DB2INST1_PASSWORD="$password" -e DBNAME=ZEUSDB \
                -e SAMPLEDB=false -e ARCHIVE_LOGS=false -e AUTOCONFIG=false \
                $image >/dev/null
        fi
        # the instance and the database are created on the first start
        i=0
        until docker logs $name 2>&1 | grep -q "Setup has completed"; do
            i=$((i + 1))
            if [ $i -gt 180 ]; then
                docker logs $name 2>&1 | tail -20
                echo "Db2 did not start" >&2
                exit 1
            fi
            sleep 5
        done
        echo "db2://db2inst1:$password@localhost:$port/ZEUSDB"
        ;;
    stop) docker rm -f $name >/dev/null ;;
    *) echo "usage: $0 start | stop" >&2; exit 2 ;;
esac
