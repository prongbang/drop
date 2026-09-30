# Drop

Rust-based HTTP server, serves any directory, inspired by `python -m http.server`.

## Using

```shell
➜ drop
   ____
  / __/__ _____  _____ ____
 _\ \/ -_) __/ |/ / -_) __/
/___/\__/_/  |___/\__/_/  (v0.2.0)

Listen on http://0.0.0.0:8000
```

## Install

### Install with the shell installer

Requires Rust and Cargo. The installer builds and installs the latest version from GitHub.

```shell
curl -fsSL https://raw.githubusercontent.com/prongbang/drop/main/install.sh | sh
```

### Install with Homebrew

```shell
brew update
brew tap prongbang/homebrew-formulae
brew install server
```

or

### Install with Cargo

```shell
cargo install drop --git https://github.com/prongbang/drop.git
```

### Load Test Report

- rust server

```shell
Running 30s test @ http://localhost:8001/user.json
  12 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency    15.23ms    6.46ms  82.74ms   69.28%
    Req/Sec     1.31k   243.95     1.99k    65.97%
  471375 requests in 30.05s, 210.84MB read
  Socket errors: connect 157, read 84, write 33, timeout 0
Requests/sec:  15688.04
Transfer/sec:      7.02MB
```

- python -m http.server

```shell
Running 30s test @ http://localhost:8000/user.json
  12 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     2.54ms    6.19ms 131.76ms   96.14%
    Req/Sec   362.26    405.00     1.66k    86.13%
  15579 requests in 30.10s, 7.21MB read
  Socket errors: connect 157, read 1148, write 146, timeout 0
Requests/sec:    517.59
Transfer/sec:    245.15KB
```
