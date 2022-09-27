# Simple Rust Blockchain
### The project I play around with when learning about new Rust concepts, patterns, etc.
---

## Current state
Auto-connecting nodes via libp2p (mDNS and Gossipsub), syncing chains on start-up & re-connect, mining blocks and broadcasting them, chain & block validation, chain persistence through Postgres

## How to run

Start the docker container in the root folder `docker-compose up -d`

Open up at least two terminals and run `cargo run {DB_NAME}`, where DB_NAME is a unique database per instance. The database has to be manually created via pg admin (user:pw @ localhost:8042)

Nodes should auto connect within a few seconds after startup. Try disconnecting any active VPN connections if this is not the case.

When debugging in VS Code: Add a database name to the args array in the launch.json file

Available commands will be shown in the terminal as soon as the app starts (e.g. block mine {BLOCK_DATA}, block validate {BLOCK_HASH}).

## Tests

Manually create a database named **blockchain_test** and run the test execution with `cargo test -- --test-threads=1`

Since the tests operate on a real database instance, the **--test-threads=1** flag is essential so there are no conflicting database calls between tests.

## Inspired by

https://github.com/zupzup/rust-blockchain-example

## Some resources I used besides official docs

### libp2p

https://github.com/Frederik-Baetens/libp2p-tokiochat/blob/main/src/main.rs

### Kademlia (DHT)

https://github.com/libp2p/specs/blob/master/kad-dht/README.md

https://medium.com/coinmonks/a-brief-overview-of-kademlia-and-its-use-in-various-decentralized-platforms-da08a7f72b8f

https://codethechange.stanford.edu/guides/guide_kademlia.html



## Design decisions
Since there is only one type (Block) stored to the DB, I decided not to use an ORM like [diesel](https://diesel.rs/) or [tokio-diesel](https://github.com/mehcode/tokio-diesel)

The blockchain and p2p libs are completely de-coupled from each other, which might not be the smartest approach for this size/kind of app. My intention here was to play around with channels


## Linking
The project uses **lld** by LLVM (available for Windows) for faster linking, which means faster (incremental) compilation. 

https://github.com/rui314/mold would be an even faster alternative (available for Linux and iOS)


## Mining

The hashing algorithm is executed in X threads in parallel (where X = available cores of the system). Benchmark tests that compare different numbers of threads and workloads per thread can be found under **benches/benchmark.rs**. Run benches with `cargo +nightly bench`


## Possible improvements (that I might or might not tackle in the future)

- [ ] store multiple messages per block and hash them into a merkle tree and store the merkle root in the block header (as Bitcoin does with transactions)
- [ ] mining: change nonce to u32 and add timestamp-refreshing each time the u32 limit (4294967295) has been unsuccessfully reached while hashing
- [ ] replace MDNS with Kademlia + Identity for peer discovery
- [ ] sync blockchain in chunks
- [ ] sync chains via a dedicated topic that is created for each sync that only the sender(s) and receiver are subscribed to
- [ ] add more tests
- [ ] set logging level on start-up
- [ ] add shell script that creates a database on start-up 
- [ ] validate a node after mining before adding it
- [ ] stop mining if a new block is added during the process