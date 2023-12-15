This repository has been archived. The work continues at https://github.com/Phala-Network/phat-quickjs.

# Sidevm QuickJS

The JS Runtime that powers the [`pink::ext().js_eval()`](https://docs.rs/pink-extension/latest/pink_extension/chain_extension/trait.PinkExtBackend.html#tymethod.js_eval) of pink-extension.

## Difference from phat-quickjs

We have developed the phat-quickjs contract, allowing any contract to execute JavaScript code. While effective, phat-quickjs has several constraints, such as limited asynchronous IO capabilities, difficulties in handling concurrent HTTP requests, and restricted memory, as it operates within the ink runtime as a standard ink contract.

In contrast, sidevm-quickjs enhances the capabilities of executing JavaScript code, pushing the boundaries further. However, it also has its limitations. Below is a comparison of the two:

| Feature | phat-quickjs | sidevm-quickjs |
|---------|--------------|----------------|
| VM Memory | 4MB | 16MB |
| Maximum Execution Time | 10 seconds | 10 seconds |
| HTTP Request API | Synchronous API | Asynchronous [fetch](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch) API |
| Concurrent Requests | Limited; through batchHttpRequest | Fully supported |
| Execution Speed | Slow | Faster |
| Blocking APIs | ✅ | ❌ |
| SCALE codec API | pink.SCALE | Sidevm.SCALE |

Notably, several APIs available in phat-quickjs are not present in sidevm-quickjs:

| API | phat-quickjs | sidevm-quickjs |
|---------|--------------|----------------|
| pink.invokeContract | ✅ | ❌ |
| pink.invokeContractDelegate | ✅ | ❌ |
| pink.httpRequest | ✅ | ❌ |
| pink.batchHttpRequest | ✅ | ❌ |
| pink.deriveSecret | ✅ | ❌ (polyfilled in [phat_js](https://docs.rs/phat_js/0.2.7/phat_js/fn.eval_async_js.html)) |
| pink.hash | ✅ | Sidevm.hash (polyfilled in [phat_js](https://docs.rs/phat_js/0.2.7/phat_js/fn.eval_async_js.html))|
| pink.vrf | ✅ | ❌ (polyfilled in [phat_js](https://docs.rs/phat_js/0.2.7/phat_js/fn.eval_async_js.html))|

And notable APIs available in sidevm-quickjs are not present in phat-quickjs:

| API | phat-quickjs | sidevm-quickjs |
|---------|--------------|----------------|
| setTimeout/setInterval | ❌ | ✅ |
| fetch | ❌ | ✅ |
| XMLHttpRequest | ❌ | ✅ |
| Promise | ❌ | ✅ |

## Performance

Currently, we have benchmarked the performance using the JS code [`examples/scale-bench.js`](./examples/scale-bench.js).
Below are the results running on a Intel(R) Core(TM) i7-9700K CPU @ 3.60GHz machine.

```
$ (cd tests/bench && cargo test --release)
2023-12-05T04:44:37.395250Z  INFO pink: JS: phat-qjs v1: parse 100 iterations in 748ms, 133.6898395721925 ops/sec    
2023-12-05T04:44:38.827895Z  INFO pink: JS: phat-qjs v1: enc/dec 100 iterations in 1424ms, 70.2247191011236 ops/sec    
2023-12-05T04:44:39.006589Z  INFO pink: JS: phat-qjs v2: parse 100 iterations in 152ms, 657.8947368421053 ops/sec    
2023-12-05T04:44:39.286615Z  INFO pink: JS: phat-qjs v2: enc/dec 100 iterations in 278ms, 359.71223021582733 ops/sec  
```

```
$ sidevm-run phatjs-opt.wasm -c @examples/scale-bench.js
2023-12-05T07:11:45.502895Z  INFO ocall{tid=0}: sidevm: JS:[2]|  Sidevm qjs: parse 10000 iterations in 1161ms, 8613.264427217915 ops/sec    
2023-12-05T07:11:47.213053Z  INFO ocall{tid=0}: sidevm: JS:[2]|  Sidevm qjs: enc/dec 10000 iterations in 1709ms, 5851.375073142189 ops/sec
```

```
$ ./target/release/phatjs examples/scale-bench.js 
2023-12-05T07:10:38.534559Z  INFO sidevm_quickjs::service: JS:[2]|  Sidevm qjs: parse 10000 iterations in 155ms, 64516.12903225806 ops/sec    
2023-12-05T07:10:38.774607Z  INFO sidevm_quickjs::service: JS:[2]|  Sidevm qjs: enc/dec 10000 iterations in 240ms, 41666.666666666664 ops/sec   
```

| Operation| Engine | iter/sec | Ratio | Ratio2 |
| --- | --- | --- | --- | --- |
| Parse | phat-quickjs v1 | 133.7 | 1x | 0.2x |
| Parse | phat-quickjs v2 | 657.9 | 5x | 1x |
| Parse | sidevm-quickjs | 8613.3 | 64x | 13x |
| Parse | quickjs-x86_64 | 64516 | 482x | 98x |
| Enc/Dec | phat-quickjs v1 | 70.2 | 1x | 0.2x |
| Enc/Dec | phat-quickjs v2 | 359.7 | 5x | 1x |
| Enc/Dec | sidevm-quickjs | 5851.4 | 83x | 16x |
| Enc/Dec | quickjs-x86_64 | 41666 | 593x | 115x |

Where phat-quickjs v1 uses pure JS SCALE codec library and others uses SCALE codec implemented in Rust in the JS Runtime.

## Build (Ubuntu 20.04)

### Prerequirements

- Some C toolchain also needed if they were not installed

  ```bash
  apt install clang clang-dev make
  ```

### Build commands

```bash
git clone https://github.com/Phala-Network/sidevm-quickjs.git --recursive
cd sidevm-quickjs
make opt
```

If no error happens, it should output `phatjs-opt.wasm` in the current directory.

```
$ ls *.wasm
phatjs-opt.wasm sidejs.wasm phatjs.wasm sidejs-opt.wasm
```
