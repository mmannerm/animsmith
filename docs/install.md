# Install

AnimSmith is one binary. Pick the route that fits your machine, then confirm
it runs.

## Prebuilt binary

Download the archive for your platform from the
[latest GitHub release](https://github.com/mmannerm/animsmith/releases/latest).
The archive names for each platform are listed in the
[CLI reference](cli.md#install). Each archive ships with a matching `.sha256`
file, so you can verify the download before unpacking it:

```console
$ sha256sum -c animsmith-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz.sha256
$ tar -xzf animsmith-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
```

Put the `animsmith` binary somewhere on your `PATH`.

## Cargo

With a Rust toolchain installed:

```console
$ cargo install animsmith
```

The default build includes FBX input and HTML reports. For a pure-Rust,
glTF-only binary with no C build step:

```console
$ cargo install animsmith --no-default-features
```

That build omits `report`, `convert`, and `assemble`; everything else works.

## Check it

```console
$ animsmith --version   # exits 0
```

## Rust pipelines

If you would rather call the checks from your own Rust code than shell out to
the binary, start with the [embedding guide](embedding.md); the crates are on
crates.io and documented on docs.rs.

The [project README](../README.md) is the same install summary in the form
crates.io and GitHub show, plus a condensed check and configuration reference.

Next: [first lint in 60 seconds](first-lint.md).
