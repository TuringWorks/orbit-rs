import os
import sys
import redis

# Redis connection parameters
REDIS_HOST = os.getenv("REDIS_HOST", "localhost")
REDIS_PORT = int(os.getenv("REDIS_PORT", 6379))
REDIS_PASSWORD = os.getenv("REDIS_PASSWORD", None)

def run_checks():
    print("Running Redis compatibility checks...")
    
    r = None
    try:
        r = redis.Redis(host=REDIS_HOST, port=REDIS_PORT, password=REDIS_PASSWORD, decode_responses=True)
        
        # 1. Connection Check
        print("1. Connection to Redis...", end=" ")
        if r.ping():
            print("PASS")
        else:
            print("FAIL")
            sys.exit(1)
        
        checks = [
            ("SET/GET", lambda: r.set("foo", "bar") and r.get("foo") == "bar"),
            ("DEL", lambda: r.delete("foo")),
            ("INCR", lambda: r.set("counter", 1) and r.incr("counter") == 2),
            ("EXPIRE", lambda: r.expire("counter", 10)),
            ("LPUSH/LPOP", lambda: r.lpush("mylist", "a") and r.lpop("mylist") == "a"),
            ("HSET/HGET", lambda: r.hset("myhash", "field1", "value1") and r.hget("myhash", "field1") == "value1"),
            ("SADD/SMEMBERS", lambda: r.sadd("myset", "member1") and "member1" in r.smembers("myset")),
            ("ZADD/ZRANGE", lambda: r.zadd("myzset", {"one": 1}) and "one" in r.zrange("myzset", 0, -1)),
            ("Pipeline", lambda: execute_pipeline(r)),
        ]
        
        failed = 0
        for name, func in checks:
            print(f"Testing {name}...", end=" ")
            try:
                if func():
                    print("PASS")
                else:
                    print("FAIL (Assertion failed)")
                    failed += 1
            except Exception as e:
                print(f"FAIL: {e}")
                failed += 1

        # Clean up
        r.flushdb()

        if failed > 0:
            print(f"\n{failed} checks failed.")
            sys.exit(1)
        else:
            print("\nAll Redis checks passed.")

    except Exception as e:
        print(f"\nCritical Error: {e}")
        sys.exit(1)

def execute_pipeline(r):
    pipe = r.pipeline()
    pipe.set('p1', 'v1')
    pipe.get('p1')
    res = pipe.execute()
    return res == [True, 'v1']

if __name__ == "__main__":
    run_checks()
