# Rust Blockchain

## 

Open up at least two terminals and run `cargo run {DB_NAME}`, where DB_NAME is a unique database per instance. The database has to be manually created via pg admin (user:pw @ localhost:8042)

Nodes should auto connect within seconds after startup. Try disabling any active VPN connections if this is not the case.

When debugging in VS Code: Add a database name to the args array in the launch.json file


## Inspired by

https://github.com/zupzup/rust-blockchain-example

## Resources

### libp2p

https://github.com/libp2p/rust-libp2p

https://github.com/Frederik-Baetens/libp2p-tokiochat/blob/main/src/main.rs

### Kademlia

https://github.com/libp2p/specs/blob/master/kad-dht/README.md

https://medium.com/coinmonks/a-brief-overview-of-kademlia-and-its-use-in-various-decentralized-platforms-da08a7f72b8f

https://codethechange.stanford.edu/guides/guide_kademlia.html



## Design decision
Since there is only one type (Block) stored to the DB, I decided not to use an ORM like [diesel](https://diesel.rs/) or [tokio-diesel](https://github.com/mehcode/tokio-diesel)


## Linking
The project uses **lld** by LLVM (available for Windows) for faster linking, which means faster (incremental) compilation. 

https://github.com/rui314/mold would be an even faster alternative (available for Linux and iOS)


## Mining

The hashing algorithm is executed in X threads in parallel (where X = available cores of the system). Benchmark tests that compare different numbers of threads and workloads per thread can be found under **benches/benchmark.rs**


## TODO

- store multiple messages per block and hash them into a merkle tree and store the merkle root in the block header
- mining: change nonce to u32 and add timestamp-refreshing each time the u32 limit (4294967295) has been unsuccessfully reached while hashing
- 