use cargo_fetch::{GitReference, Package, PackageFetcher, PackageSource};

fn main() {
    let mut fetcher = PackageFetcher::new().expect("failed to construct the fetcher");

    let custom_source = PackageSource::remote("https://github.com/rust-lang/crates.io-index").expect("bad url");
    let git_source = PackageSource::git(
        "https://github.com/serde-rs/serde",
        Some(GitReference::Tag("v1.0.0".into())),
    )
    .expect("bad url");

    // Same as `serde = "*"` in Cargo.toml
    let crates_io = fetcher.resolve_first("serde", None, PackageSource::CratesIo, None).expect("can't find serde");
    let custom_registry = Package::from_str_ver("serde", "1.0.0", &custom_source).expect("bad semver");
    let git = Package::from_str_ver(
        "serde",     // name
        "1.0.0",     // version
        &git_source, // source
    )
    .expect("bad semver");

    let serde_roots = fetcher
        .fetch_many(&[git, crates_io, custom_registry])
        .expect("failed to fetch packages");

    println!("serde_roots={serde_roots:#?}");
}
