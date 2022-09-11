# Rust Blockchain




Inspired by

https://github.com/zupzup/rust-blockchain-example

Resources

libp2p

https://github.com/libp2p/rust-libp2p

https://github.com/Frederik-Baetens/libp2p-tokiochat/blob/main/src/main.rs

Kademlia

https://github.com/libp2p/specs/blob/master/kad-dht/README.md

https://medium.com/coinmonks/a-brief-overview-of-kademlia-and-its-use-in-various-decentralized-platforms-da08a7f72b8f

https://codethechange.stanford.edu/guides/guide_kademlia.html




## Linking
The project uses **lld** by LLVM for faster linking (under Windows), which means faster (incremental) compilation. 

https://github.com/rui314/mold would be an even faster alternative (only available for Linux and iOS though)


## Mining

The hashing algorithm is executed in X threads in parallel (where X = available cores of the system). Benchmark tests that compare different numbers of threads and workloads per thread can be found under **benches/benchmark.rs**
