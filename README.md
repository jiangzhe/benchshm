# Benchshm

## Background

This is a simple project to benchmark TCP, Unix Socket and shared Memory performance on local machine:

It focuses on latency of basic RPC pattern: 1 request 1 response.

The request/response pattern is commonly used in many scenarios: REST api,
database interface, etc.

The benchmark runs on single thread.

## Environment and Result

Intel(R) Xeon(R) Silver 4214 CPU @ 2.20GHz

- TCP

```shell
# server
[benchshm]# ./target/release/svr --addr=tcp:127.0.0.1:9001
Listening at (Tcp)(127.0.0.1:9001)
disconnected from remote addr 127.0.0.1:39516, sum is 100000, duration is 1.723617201s
disconnected from remote addr 127.0.0.1:39910, sum is 100000, duration is 1.734910858s
disconnected from remote addr 127.0.0.1:40372, sum is 100000, duration is 1.72155573s
disconnected from remote addr 127.0.0.1:41114, sum is 200000, duration is 70.807929ms
disconnected from remote addr 127.0.0.1:41312, sum is 200000, duration is 69.085267ms
disconnected from remote addr 127.0.0.1:41430, sum is 200000, duration is 70.663939ms

# client
[benchshm]# ./target/release/cli --addr=tcp:127.0.0.1:9001 -n 100000 -v 1
connecting (Tcp)(127.0.0.1:9001)
disconnected: num is 100000, sum is 100000, duration is 1.723606599s, avg latency is 17.236µs
[benchshm]# ./target/release/cli --addr=tcp:127.0.0.1:9001 -n 100000 -v 1
connecting (Tcp)(127.0.0.1:9001)
disconnected: num is 100000, sum is 100000, duration is 1.734892777s, avg latency is 17.348µs
[benchshm]# ./target/release/cli --addr=tcp:127.0.0.1:9001 -n 100000 -v 1
connecting (Tcp)(127.0.0.1:9001)
disconnected: num is 100000, sum is 100000, duration is 1.721537026s, avg latency is 17.215µs
[benchshm]# ./target/release/cli --addr=tcp:127.0.0.1:9001 -n 100000 -v 2
connecting (Tcp)(127.0.0.1:9001)
disconnected: num is 100000, sum is 200000, duration is 67.025329ms, avg latency is 670ns
[benchshm]# ./target/release/cli --addr=tcp:127.0.0.1:9001 -n 100000 -v 2
connecting (Tcp)(127.0.0.1:9001)
disconnected: num is 100000, sum is 200000, duration is 66.305733ms, avg latency is 663ns
[benchshm]# ./target/release/cli --addr=tcp:127.0.0.1:9001 -n 100000 -v 2
connecting (Tcp)(127.0.0.1:9001)
disconnected: num is 100000, sum is 200000, duration is 67.831384ms, avg latency is 678ns

```

- Unix Socket

```shell
# server
[benchshm]# ./target/release/svr --addr=unix:./unix.sock
Listening at (Unix)(./unix.sock)
disconnected from remote addr (unnamed), sum is 100000, duration is 828.766224ms
disconnected from remote addr (unnamed), sum is 100000, duration is 837.966101ms
disconnected from remote addr (unnamed), sum is 100000, duration is 837.46609ms
disconnected from remote addr (unnamed), sum is 200000, duration is 118.128449ms
disconnected from remote addr (unnamed), sum is 200000, duration is 111.737795ms
disconnected from remote addr (unnamed), sum is 200000, duration is 114.25196ms

# client
[benchshm]# ./target/release/cli --addr=unix:./unix.sock -n 100000 -v 1
connecting (Unix)(./unix.sock)
disconnected: num is 100000, sum is 100000, duration is 828.771361ms, avg latency is 8.287µs
[benchshm]# ./target/release/cli --addr=unix:./unix.sock -n 100000 -v 1
connecting (Unix)(./unix.sock)
disconnected: num is 100000, sum is 100000, duration is 837.96211ms, avg latency is 8.379µs
[benchshm]# ./target/release/cli --addr=unix:./unix.sock -n 100000 -v 1
connecting (Unix)(./unix.sock)
disconnected: num is 100000, sum is 100000, duration is 837.456928ms, avg latency is 8.374µs
[benchshm]# ./target/release/cli --addr=unix:./unix.sock -n 100000 -v 2
connecting (Unix)(./unix.sock)
disconnected: num is 100000, sum is 200000, duration is 118.127061ms, avg latency is 1.181µs
[benchshm]# ./target/release/cli --addr=unix:./unix.sock -n 100000 -v 2
connecting (Unix)(./unix.sock)
disconnected: num is 100000, sum is 200000, duration is 111.73202ms, avg latency is 1.117µs
[benchshm]# ./target/release/cli --addr=unix:./unix.sock -n 100000 -v 2
connecting (Unix)(./unix.sock)
disconnected: num is 100000, sum is 200000, duration is 114.24571ms, avg latency is 1.142µs

```

- Shared memory

```shell
# server
[benchshm]# ./target/release/svr --addr=shm:./shm.flk
Listening at (Shm)(./shm.flk)
disconnected from client 2720927857, sum is 4999950000, duration is 26.5511ms
disconnected from client 433334254, sum is 4999950000, duration is 27.986401ms
disconnected from client 3319983730, sum is 4999950000, duration is 36.142223ms
disconnected from client 921933548, sum is 4999950000, duration is 30.704631ms
disconnected from client 2786059088, sum is 4999950000, duration is 36.225714ms
disconnected from client 3454974945, sum is 4999950000, duration is 27.58407ms

# client
[benchshm]# ./target/release/cli --addr=shm:./shm.flk -n 100000 -v 1
connecting (Shm)(./shm.flk)
disconnected: num is 100000, sum is 4999950000, duration is 26.552681ms, avg latency is 265ns
[benchshm]# ./target/release/cli --addr=shm:./shm.flk -n 100000 -v 1
connecting (Shm)(./shm.flk)
disconnected: num is 100000, sum is 4999950000, duration is 27.98815ms, avg latency is 279ns
[benchshm]# ./target/release/cli --addr=shm:./shm.flk -n 100000 -v 1
connecting (Shm)(./shm.flk)
disconnected: num is 100000, sum is 4999950000, duration is 36.143489ms, avg latency is 361ns
[benchshm]# ./target/release/cli --addr=shm:./shm.flk -n 100000 -v 2
connecting (Shm)(./shm.flk)
disconnected: num is 100000, sum is 4999950000, duration is 30.706302ms, avg latency is 307ns
[benchshm]# ./target/release/cli --addr=shm:./shm.flk -n 100000 -v 2
connecting (Shm)(./shm.flk)
disconnected: num is 100000, sum is 4999950000, duration is 36.226917ms, avg latency is 362ns
[benchshm]# ./target/release/cli --addr=shm:./shm.flk -n 100000 -v 2
connecting (Shm)(./shm.flk)
disconnected: num is 100000, sum is 4999950000, duration is 27.585196ms, avg latency is 275ns

```

Notes: 

`v1`: sync response per request(in another word, no pipeline).

`v2`: send requests only, do not wait for response.

In case of shared memory, `v2` is identical to `v1`.

## Conclusion

Shared memory is faster than both TCP and Unix Socket by two orders of magnitude.

Unix Socket is slightly faster than TCP.
