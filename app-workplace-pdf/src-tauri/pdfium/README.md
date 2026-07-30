# PDFium runtime binaries

`pdfium-render` binds Google's PDFium C library at runtime. The app searches
for the platform library in this order (see `src/main.rs` `init_pdfium()`):

1. `$PDFIUM_DYNAMIC_LIB_PATH` (directory containing the library)
2. the executable's own directory
3. `../Frameworks` relative to the executable (macOS `.app` bundle layout)
4. `./pdfium/` relative to the CWD — i.e. this directory when running
   `cargo run` / `cargo tauri dev` from `src-tauri/`
5. the system library search path

Binaries in this directory are **not committed** (see `../../.gitignore`).
Source: <https://github.com/bblanchon/pdfium-binaries> (Apache-2.0 — clean to
bundle per CLAUDE.md).

## Currently fetched (local dev only)

| File | Platform | Release | sha256 |
|---|---|---|---|
| `libpdfium.so` | linux-x64 | `chromium/7947` (PDFium 152.0.7947.0, fetched 2026-07-13) | `61c9f745c6296a1050599a99a1ed985036411b591a11bd2a41bafe530ecb4f33` |

Archive sha256 (`pdfium-linux-x64.tgz`):
`f73d69d309fe1f33cc7269dcc99be31ec44e1cf608e31d7e2fcc6545fc2f9323`

## macOS (the actual ship target) — still needed

The shipped app targets macOS 10.13 (`minimumSystemVersion`), and CLAUDE.md
requires the PDFium binary to be **statically linked** for macOS distribution.
On the macOS build machine either:

- **Static (required for release):** obtain/build `libpdfium.a` and enable
  pdfium-render's `static` feature in `Cargo.toml`
  (plus `libstdc++`/`libc++`/`core_graphics` feature as appropriate); or
- **Dynamic (dev convenience only):** download
  `pdfium-mac-univ.tgz` (or `-arm64`/`-x64`) from the same release and place
  `libpdfium.dylib` in this directory.

Note: bblanchon prebuilt dylibs advertise a newer macOS deployment target than
10.13 — verify with `otool -l libpdfium.dylib | grep -A2 LC_VERSION_MIN` (or
`minos`) on the build machine; a custom PDFium build may be needed to honour
the 10.13 floor.
