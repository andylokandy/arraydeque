# `arraydeque`

[![build status](https://travis-ci.org/andylokandy/arraydeque.svg?branch=master)](https://travis-ci.org/andylokandy/arraydeque)
[![codecov](https://codecov.io/gh/andylokandy/arraydeque/branch/master/graph/badge.svg)](https://codecov.io/gh/andylokandy/arraydeque)
[![crates.io](https://img.shields.io/crates/v/arraydeque.svg)](https://crates.io/crates/arraydeque)
[![docs.rs](https://docs.rs/arraydeque/badge.svg)](https://docs.rs/arraydeque)

A circular buffer with fixed capacity.  Requires Rust 1.20+.

This crate is inspired by [**bluss/arrayvec**](https://github.com/bluss/arrayvec)

### [**Documentation**](https://docs.rs/arraydeque)

## Usage

First, add the following to your `Cargo.toml`:

```toml
[dependencies]
arraydeque = "0.3"
```

Next, add this to your crate root:

```rust
extern crate arraydeque;
```

Currently arraydeque by default links to the standard library, but if you would
instead like to use arraydeque in a `#![no_std]` situation or crate you can
request this via:

```toml
[dependencies]
arraydeque = { version = "0.4", default-features = false }
```

## Example

```rust
extern crate arraydeque;

use arraydeque::ArrayDeque;

fn main() {
    let mut deque: ArrayDeque<[_; 2]> = ArrayDeque::new();
    assert_eq!(deque.capacity(), 2);
    assert_eq!(deque.len(), 0);

    deque.push_back(1);
    deque.push_back(2);
    assert_eq!(deque.len(), 2);

    assert_eq!(deque.pop_front(), Some(1));
    assert_eq!(deque.pop_front(), Some(2));
    assert_eq!(deque.pop_front(), None);
}
```

## Contribution

All kinds of contribution are welcomed.

- **Issus.** Feel free to open an issue when you find typos, bugs, or have any question.
- **Pull requests**. New collection, better implementation, more tests, more documents and typo fixes are all welcomed.

## License

Licensed under MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
