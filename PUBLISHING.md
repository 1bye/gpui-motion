# Release preparation

Packages: `gpui-ce-motion` and `gpui-motion-kit` · repository: https://github.com/1bye/gpui-motion

The GitHub CI workflow validates changes only; it has no publishing step or
crates.io credentials.

## Validate a release

Use Rust 1.95 or newer. Native examples require the host GPUI CE prerequisites
(on macOS, Xcode command-line tools and a graphical login session).

```sh
cargo fmt --all --check
cargo test --locked --no-default-features
cargo test --locked --no-default-features --features text
cargo test --locked --features gpui
cargo test --locked --features demo,native-tests
cargo test --locked --features demo,native-tests --example motion_showcase
cargo clippy --locked --all-targets --features demo,native-tests -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --features gpui
cargo build --locked --release --features demo --examples
cargo bench --locked --bench sampling
cargo bench --locked --features native-tests --bench layout_native
cargo bench --locked --features native-tests --bench advanced_native
cargo bench --locked --features text --bench text_sampling
cargo bench --locked --features native-tests --bench text_native
cargo package --list
cargo publish --dry-run --locked --features gpui
```

Run the showcase with `cargo run --release --features demo --example motion_showcase`.
Check interaction, reduced motion, and presentation on each claimed platform.
Native TestPlatform tests and CPU benchmarks do not verify GPU presentation.
`PERFORMANCE.md` distinguishes historical measurements from extraction checks.

The include list contains the library, examples and their assets, tests,
benchmarks, documentation and licenses. Inspect it before each release. Verify
that the packaged archive builds independently of the source checkout, including
`cargo check --features demo --examples` from an extracted archive. All dependencies
resolve from crates.io; no sibling checkout or Git patch is required.

## Publish manually, only when ready

1. Check the published crates.io versions and choose an unused next version.
2. Choose the version in Cargo.toml, review changes, commit, and pass CI.
3. Sign into crates.io, verify your email, and create a scoped API token.
4. Run `cargo login` locally and paste the token at its prompt.
5. Run the dry-run command above, then explicitly run:

```sh
cargo publish --locked --features gpui
```

Only this last command uploads the crate. Published versions cannot be overwritten.
After successful publication, create the matching Git tag and GitHub release,
and confirm that docs.rs built the native API documentation. Update the
changelog and README installation instructions as part of release preparation.

Official guide: https://doc.rust-lang.org/cargo/reference/publishing.html

## Consumers

For unreleased development, use a path to this checkout or a Git revision:

```toml
[dependencies]
gpui-ce-motion = { git = "https://github.com/1bye/gpui-motion", rev = "<commit>", features = ["gpui"] }
gpui = { package = "gpui-ce", version = "=0.2.2", default-features = false }
```

For registry releases:

```toml
[dependencies]
gpui-ce-motion = { version = "0.2", features = ["gpui"] }
```

Use the same GPUI CE source and version throughout the consuming application.
Enable `demo` only to run this repository's examples; it adds platform backends.
Keep `native-tests` for development checks. The default build is portable and
has no third-party dependencies.

## GPUI Kit adapter

`integrations/gpui-kit` is an independently published crate using the portable
engine without its CE-native feature. Keep its version aligned with the core;
publish a required new core version before publishing the adapter. Its dependency
includes both a development path and a registry version, so Cargo resolves the
published core when verifying the archive.

From the repository root:

```sh
cargo fmt --manifest-path integrations/gpui-kit/Cargo.toml --all --check
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests --example showcase
cargo test --manifest-path integrations/gpui-kit/Cargo.toml --locked --features demo,native-tests --example text_motion
cargo clippy --manifest-path integrations/gpui-kit/Cargo.toml --locked --all-targets --features demo,native-tests -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path integrations/gpui-kit/Cargo.toml --locked --no-deps
python3 scripts/check_spacing.py --project integrations/gpui-kit
cargo package --manifest-path integrations/gpui-kit/Cargo.toml --list
cargo publish --manifest-path integrations/gpui-kit/Cargo.toml --dry-run --locked
cargo check --locked --manifest-path integrations/gpui-kit/target/package/gpui-motion-kit-0.2.0/Cargo.toml --target-dir integrations/gpui-kit/target --features demo --examples
```

The archive includes the library, README, license notices, demos, and demo assets.
Native tests and benchmarks reuse fixtures outside the adapter folder and remain
in the Git repository. The archive's example check verifies that no sibling
checkout is needed. CI runs that check and compiles both READMEs against their
respective backends. Docs.rs uses the Linux target covered by the docs CI job.

After the release commit passes CI, publish explicitly:

```sh
cargo publish --manifest-path integrations/gpui-kit/Cargo.toml --locked
```

For adapter-only releases, use a distinct `gpui-motion-kit-v<version>` tag so it
does not move the core's existing `v<version>` tag. Verify a registry-only Kit
consumer and the hosted Kit documentation after publishing.
