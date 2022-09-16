#![feature(test)]
extern crate rust_blockchain;
use rust_blockchain::blockchain as blockchain;
extern crate test;

use test::Bencher;

/* 
difficulty "00"

blocks of 1000 nonces per iteration per thread
test test_hashing_multithreaded ... bench: 363,527,470 ns/iter (+/- 179,152,812)
test test_hashing_single_thread ... bench: 1,069,756,730 ns/iter (+/- 175,252,846)
test test_hashing_two_threads   ... bench: 605,743,500 ns/iter (+/- 144,547,350)

blocks of 100 nonces per iteration per thread
test test_hashing_multithreaded ... bench: 278,368,280 ns/iter (+/- 60,577,935)
test test_hashing_single_thread ... bench: 969,069,310 ns/iter (+/- 69,843,948)
test test_hashing_two_threads   ... bench: 535,899,490 ns/iter (+/- 57,942,930)

blocks of 10 nonces per iteration per thread
test test_hashing_multithreaded ... bench: 287,252,090 ns/iter (+/- 75,705,529)
test test_hashing_single_thread ... bench: 962,710,710 ns/iter (+/- 86,885,020)
test test_hashing_two_threads   ... bench: 565,976,550 ns/iter (+/- 101,239,246)

blocks of 1 nonce per iteration per thread
test test_hashing_multithreaded ... bench: 306,904,680 ns/iter (+/- 105,593,722)
test test_hashing_single_thread ... bench: 1,023,337,880 ns/iter (+/- 105,662,968)
test test_hashing_two_threads   ... bench: 590,224,610 ns/iter (+/- 89,081,472)

synchronous hashing thread
test test_hashing_sync ... bench: 961,240,210 ns/iter (+/- 102,724,215)

*/

#[bench]
fn test_hashing_single_thread(b: &mut Bencher) {
    b.iter(|| blockchain::find_hash("prev_hash", "data", 1234545678, "00", 1 as usize));
}

#[bench]
fn test_hashing_two_threads(b: &mut Bencher) {
    b.iter(|| blockchain::find_hash("prev_hash", "data", 1234545678, "00", 2 as usize));
}

#[bench]
fn test_hashing_multithreaded(b: &mut Bencher) {
    let threads = num_cpus::get();
    b.iter(|| blockchain::find_hash("prev_hash", "data", 1234545678, "00", threads));
}

#[bench]
fn test_hashing_sync(b: &mut Bencher) {
    b.iter(|| blockchain::find_hash_sync("prev_hash", "data", 1234545678, "00"));
}
