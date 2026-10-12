#!/bin/sh
# IBM MQ for Zeus: the queue manager in a container (the IBM MQ Advanced
# for Developers image, from IBM's registry) and the IBM MQ C client
# library (the redistributable client, from IBM's download site). Neither
# is part of Zeus.
#
# usage: tools/mq.sh client   # downloads the client; prints ZEUS_MQ_LIB
#        tools/mq.sh start    # prints the ZEUS_MQ URL when the queue manager is up
#        tools/mq.sh stop
#
# The queue manager is QM1 on port ${ZEUS_MQ_PORT:-1414}; applications
# connect as user app through channel DEV.APP.SVRCONN and use the queues
# DEV.QUEUE.1 to DEV.QUEUE.3.
set -eu

name=zeus-mq
image=icr.io/ibm-messaging/mq:latest
port=${ZEUS_MQ_PORT:-1414}
password=${ZEUS_MQ_PASSWORD:-zeusapp1}
version=${ZEUS_MQ_CLIENT:-9.4.5.1}
dir=$(cd "$(dirname "$0")/.." && pwd)/.cache/mq

case ${1:-} in
    client)
        if [ ! -f "$dir/redist/lib64/libmqic_r.so" ]; then
            mkdir -p "$dir/redist"
            tgz=$version-IBM-MQC-Redist-LinuxX64.tar.gz
            curl -sSf -o "$dir/$tgz" \
                "https://public.dhe.ibm.com/ibmdl/export/pub/software/websphere/messaging/mqdev/redist/$tgz"
            tar xzf "$dir/$tgz" -C "$dir/redist"
        fi
        echo "$dir/redist/lib64"
        ;;
    start)
        since=$(date -u +%Y-%m-%dT%H:%M:%SZ)
        if docker ps --format '{{.Names}}' | grep -qx $name; then
            since=1970-01-01T00:00:00Z
        elif docker ps -a --format '{{.Names}}' | grep -qx $name; then
            docker start $name >/dev/null
        else
            docker run -d --name $name -p "$port:1414" \
                -e LICENSE=accept -e MQ_QMGR_NAME=QM1 \
                -e MQ_APP_PASSWORD="$password" \
                -e MQ_ADMIN_PASSWORD="${ZEUS_MQ_ADMIN_PASSWORD:-zeusadm1}" \
                $image >/dev/null
        fi
        i=0
        until docker logs --since "$since" $name 2>&1 | grep -aq "AMQ5026I\|AMQ8024I"; do
            i=$((i + 1))
            if [ $i -gt 120 ]; then
                docker logs $name 2>&1 | tail -20
                echo "the queue manager did not start" >&2
                exit 1
            fi
            sleep 2
        done
        echo "mq://app:$password@localhost:$port/QM1/DEV.APP.SVRCONN"
        ;;
    stop) docker rm -f $name >/dev/null ;;
    *) echo "usage: $0 client | start | stop" >&2; exit 2 ;;
esac
