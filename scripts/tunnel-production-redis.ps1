# SSH local port forward: 127.0.0.1:6379 -> northcloud.biz Redis.
# Leave this running; in another terminal set REDIS_ADDR, REDIS_PASSWORD, REDIS_CHANNELS and run the app.
# See docs/PRODUCTION_REDIS.md.

$ErrorActionPreference = "Stop"
ssh -L 6379:localhost:6379 -N jones@northcloud.biz
