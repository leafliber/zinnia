#!/bin/sh
set -e

# Load password from secret file
if [ -f "/run/secrets/redis_password" ]; then
    REDIS_PASSWORD=$(cat /run/secrets/redis_password)
else
    echo "Redis password secret not found!"
    exit 1
fi

# Run Redis with arguments
# Note: passing password via command line is visible in process list, 
# but inside container this is acceptable/standard for redis images 
# when not using config file.
exec redis-server \
    --requirepass "$REDIS_PASSWORD" \
    --appendonly yes \
    --maxmemory 512mb \
    --maxmemory-policy allkeys-lru \
    --save 900 1 \
    --save 300 10 \
    --save 60 10000