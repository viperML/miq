<h1 align="center">miq - Immutable package management for Linux</h1>

| Thesis Document | Binary release | Source code |
| --- | --- | --- |
| [PDF](https://github.com/viperML/miq/releases/download/latest/index.pdf) | [tarball](https://github.com/viperML/miq/releases/download/latest/release.tar.gz) | [tarball](https://github.com/viperML/miq/archive/master.tar.gz) |


This repository holds the work for my Master's thesis at the University of Cádiz for my Master's Degree in Computer and Systems Engineering Research.

Miq is a package manager for Linux written in Rust, which uses the same concept as [Nix](https://nixos.org) for package deployment.
Each package, which is built from source, receives a unique ID based on its dependencies and configuration, which is used as the path to the package in the file system. This allows for multiple versions of the same package to coexist in the file system, instead of having to replace the contents with an update in `/usr`. For this reason, packages are not "mutated" in disk, but when a package is built with a different dependency or an update, it gets a different path.

```
/miq/store/unpack-bootstrap-tools.sh-6949dd1f64cfe7b6
/miq/store/busybox-33a90b67a497c4d6
/miq/store/toybox-x86_64-69a4327d80d88104
/miq/store/bootstrap-tools.tar.xz-9d678d0fc5041f17
```

The package manager implements a "recipe" evaluator implemented in Lua, which is used to describe how packages are built (similarly to ebuilds or rpmspec). This evaluator also calculates the hashes of the packages, and lets the user define the packages without having to hardcode these values.


<!-- <p align="center">
  <a href="https://github.com/viperML/miq/actions/workflows/release.yaml">
  <img alt="build: passing" src="https://img.shields.io/github/actions/workflow/status/viperML/miq/ci.yaml?branch=master&label=release">
  </a>
</p> -->

## Installation and usage

Miq is distributed as a statically linked ELF with no dependencies, meaning it should work on any host Linux distribution with no dependencies.

- (Optional) Make a new working directory:
  ```sh
  mkdir ~/miq && cd ~/miq
  ```
- Download and unpack the release tarball:
  ```sh
  curl -OL https://github.com/viperML/miq/releases/download/latest/release.tar.gz
  tar -xvf release.tar.gz
  ```
- If bubblewrap is not installed in the host system, add it to `PATH`:
  ```sh
  export PATH="$PWD:$PATH"
  ```
- Run miq:
  ```sh
  miq --help
  ```

The usage manual is rendered on the thesis document available for download.

## Development

Nix is used to provide the Rust toolchain and the required development dependencies, like rust-analyzer and sqlite3. Once nix is properly installed, getting a devshell for development is as simple as running:

```sh
nix develop
cargo build
```


## License

- Source code is [EUPL v1.2](https://eupl.eu/1.2/en)
- Documentation ([./doc](./doc)) is [CC BY NC SA](https://creativecommons.org/licenses/by-nc-sa/4.0)
