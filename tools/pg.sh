#!/bin/sh
# Starts PostgreSQL in a container for Zeus (the official postgres image),
# a database server for EXEC SQL instead of Db2.
#
# usage: tools/pg.sh start   # prints the ZEUS_DB2 URL when the server is up
#        tools/pg.sh start-ebcdic  # the same, for a database with EBCDIC collation
#        tools/pg.sh stop
#
# The database is zeusdb, user zeus, on port ${ZEUS_PG_PORT:-5432}.
set -eu

name=zeus-pg
image=postgres:16
port=${ZEUS_PG_PORT:-5432}
password=${ZEUS_PG_PASSWORD:-zeuspass}

up() {
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
}

case ${1:-} in
    start)
        up
        echo "postgres://zeus:$password@localhost:$port/zeusdb"
        ;;
    start-ebcdic)
        # database zeusebcdic: text ordered as in EBCDIC code page 037
        # (blank, special characters, lowercase, uppercase, digits), by ICU
        # rules that list the characters of 037 in code point order
        up
        if ! docker exec $name psql -U zeus -d postgres -Atc \
            "SELECT 1 FROM pg_database WHERE datname = 'zeusebcdic'" | grep -q 1; then
            rules=$(sed "s/'/''/g" "$(dirname "$0")/pg-ebcdic037.rules")
            docker exec $name psql -q -U zeus -d postgres -c \
                "CREATE DATABASE zeusebcdic TEMPLATE template0 LOCALE_PROVIDER icu ICU_LOCALE 'und' ICU_RULES '$rules'" >/dev/null
        fi
        order=$(docker exec $name psql -U zeus -d zeusebcdic -Atc \
            "SELECT string_agg(x, '' ORDER BY x) FROM (VALUES ('1'), ('A'), ('a'), ('#')) t(x)")
        if [ "$order" != "#aA1" ]; then
            echo "zeusebcdic does not order text as EBCDIC ($order)" >&2
            exit 1
        fi
        echo "postgres://zeus:$password@localhost:$port/zeusebcdic"
        ;;
    stop) docker rm -f $name >/dev/null ;;
    *) echo "usage: $0 start | start-ebcdic | stop" >&2; exit 2 ;;
esac
