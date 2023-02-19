## `cargo_fetch`

#### What?

A library that aims to provide an easy and stable API for tools to fetch packages.
Uses `cargo` library under the hood.

#### Why?

Using `cargo` library allows it to reuse a lot of code, that would be
hard and tedious to reimplement.
This includes:
- Using the `cargo` cache (`$HOME/.cargo`),
- Supporting every package source that `cargo` does: remote, local registries, git repositories, local paths.

Reusing the `cargo` code also provides reliability and security.

#### How?

Using this library is pretty simple:

```rust
use cargo_fetch::{GitReference, Package, PackageFetcher, PackageSource};

fn main() {
    let mut fetcher = PackageFetcher::default().expect("failed to construct the fetcher");

    let custom_source = PackageSource::remote("https://github.com/rust-lang/crates.io-index").expect("bad url");
    let git_source = PackageSource::git(
        "https://github.com/serde-rs/serde",
        Some(GitReference::Tag("v1.0.0".into())),
    )
    .expect("bad url");

    let crates_io = fetcher.resolve_package("serde", "*", PackageSource::CratesIo, None).expect("can't find serde");
    let custom_registry = Package::from_str_ver("serde", "1.0.0", &custom_source).expect("bad semver");
    let git = Package::from_str_ver(
        "serde",     // name
        "1.0.0",     // version
        &git_source, // source
    )
    .expect("bad semver");

    let serde_roots = fetcher
        .fetch_many(&[git, crates_io, custom_registry], None)
        .expect("failed to fetch packages");

    println!("serde_roots={serde_roots:#?}");
}
```

You can run this example with `cargo run --example fetch_serde` in the root of this repository.

#### License
Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
[https://www.apache.org/licenses/LICENSE-2.0](https://www.apache.org/licenses/LICENSE-2.0)> or the MIT license
<LICENSE-MIT or [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)>, at your
option. Files in the project may not be copied, modified, or distributed except according to those terms.
