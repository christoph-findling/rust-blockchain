# Rust Blockchain

## 

Open up at least two terminals and run `dasdasd`

Nodes should auto connect within seconds after startup. Try disabling any active VPN connections if this is not the case.




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




## Linking
The project uses **lld** by LLVM (available for Windows) for faster linking, which means faster (incremental) compilation. 

https://github.com/rui314/mold would be an even faster alternative (available for Linux and iOS)


## Mining

The hashing algorithm is executed in X threads in parallel (where X = available cores of the system). Benchmark tests that compare different numbers of threads and workloads per thread can be found under **benches/benchmark.rs**
