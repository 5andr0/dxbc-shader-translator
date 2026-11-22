# dxbc-shader-translator

`d2r-shader-translator` is a shader translation service. It takes compiled Direct3D shader bytecode (DXBC) and
translates it to WGSL and SPIR-V for a wgpu renderer. The work is done by the
[iridia](https://crates.io/crates/iridia) crates (DXBC lifting) and naga (WGSL / SPIR-V output). The crates in this
repository prepare the inputs for those crates, check the results, and answer the renderer's requests.

The program runs as a separate process. A renderer starts it, writes a request file into a cache folder and reads the
result file back. The renderer does not link this program.

The program contains no game data: no shader bytecode, no translated shader text and no tables copied from game files.
Every shader it translates comes from the player's own game installation at run time, passed in by the renderer.
Diablo II: Resurrected is a trademark of Blizzard Entertainment, Inc. This project is not affiliated with or endorsed by
Blizzard Entertainment.

## Usage

```
d2r-shader-translator run <cache root> <request.req.json>   translate one request, write its result next to it
d2r-shader-translator warm <cache root> [--until-stdin-closes]   translate every logged request that has no result yet
                                                                 (with the flag: exit as soon as stdin closes)
d2r-shader-translator verify <cache root>   run every cached request again and compare the results byte for byte
d2r-shader-translator cache-id | identity   print the cache id / the program identity
```

Results are cached under `<cache root>/t-<cache id>/`. The cache id is a hash of everything that decides the output:
the source files of every path crate in the build, the locked versions of every registry crate, the toolchain and the
target settings.

## Repository layout

| Path | Contents |
|---|---|
| `crates/d2r-shader-translator` | the program (`src/main.rs`); `build.rs` computes the build part of the cache id |
| `crates/d2r-shader`, `crates/d2r-shader-client` | shader data definitions, request and result forms, the result cache |
| `crates/d2r-shader-iridia`, `tools/iridia-terrain53` | the crates that drive iridia and naga |
| `patches/` | edits to five crates.io crates (`*.edits.json`), the SHA-256 of the official archives they apply to (`dependencies.json`) and the tool that applies them (`patched_crates.py`) |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `rust-toolchain.toml` | workspace, locked versions, build flags, toolchain |
| `LICENSE`, `LICENSE-MIT`, `NOTICES.md`, `licences/` | licence texts and the licences of the linked crates |

## Build

Windows x64 with the MSVC build tools, Python 3 and the Rust toolchain named in `rust-toolchain.toml` (rustup installs
it automatically):

```
python patches/patched_crates.py --download
cargo build --release -p d2r-shader-translator
```

The first command downloads the five pinned crates.io archives, checks their SHA-256, applies the edits in `patches/`
and writes the patched crates to `.patched/`, where the `[patch.crates-io]` block of `Cargo.toml` points. The program is
`target/x86_64-pc-windows-msvc/release/d2r-shader-translator.exe`. Run the first command before the first build and again after `patches/` changes: three source files include patched iridia files from `.patched/`, so the build fails without them.

The binary distribution is built the same way with `--profile iter` (defined in `Cargo.toml`) instead of `--release`.

## Licence

The program as a whole is licensed under the GNU General Public License, version 3 or (at your option) any later
version (`LICENSE`), because it links the GPL-3.0-or-later iridia crates. `crates/d2r-shader-iridia` and
`tools/iridia-terrain53` are GPL-3.0-or-later. `crates/d2r-shader`, `crates/d2r-shader-client` and the project's code in
`crates/d2r-shader-translator` are also available under the MIT licence (`LICENSE-MIT`). Third-party crates keep their
own licences: `licences/rust-crates.txt` lists every linked crate with its licence; `licences/rust-crate-licenses.txt`
and `licences/standard-licences/` hold the texts. `NOTICES.md` describes the patched crates.
