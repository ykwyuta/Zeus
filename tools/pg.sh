#!/bin/sh
# Starts PostgreSQL in a container for Zeus (the official postgres image),
# a database server for EXEC SQL instead of Db2.
#
# usage: tools/pg.sh start   # prints the ZEUS_DB2 URL when the server is up
#        tools/pg.sh stop
#
# The database is zeusdb, user zeus, on port ${ZEUS_PG_PORT:-5432}.
set -eu

name=zeus-pg
image=postgres:16
port=${ZEUS_PG_PORT:-5432}
password=${ZEUS_PG_PASSWORD:-zeuspass}

case ${1:-} in
    start)
        if ! docker ps --format '{{.Names}}' | grep -qx $name; then
            if docker ps -a --format '{{.Names}}' | grep -qx $name; then
                docker start $name >/dev/null
            else
                docker run -d --name $name -p "$port:5432" \
                    -e POSTGRES_USER=zeus -e POSTGRES_PASSWORD="$password" \
                    -e POSTGRES_DB=zeusdb $image >/dev/null
            fi
        fi
        i=0
        until docker exec $name pg_isready -q -U zeus -d zeusdb -h localhost 2>/dev/null; do
            i=$((i + 1))
            if [ $i -gt 60 ]; then
                docker logs $name 2>&1 | tail -20
                echo "PostgreSQL did not start" >&2
                exit 1
            fi
            sleep 1
        done
        echo "postgres://zeus:$password@localhost:$port/zeusdb"
        ;;
    stop) docker rm -f $name >/dev/null ;;
    *) echo "usage: $0 start | stop" >&2; exit 2 ;;
esac
