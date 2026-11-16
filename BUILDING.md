# Building Gino from source

For using Gino you do not need any of this: download the installer from the [releases page](https://github.com/gmleuning/gino/releases/latest).

## What you need

- Rust, stable, 1.92 or later
- The Microsoft C++ Build Tools ("Desktop development with C++")
- The Edge WebView2 runtime (already on current Windows)
- Node.js 22 or later

## Commands

```powershell
npm install
npm run tauri dev       # run with live reload
npm run tauri build     # installer in target\release\bundle\nsis\
cargo test -p ginos-core
npm run check           # type-check the interface
```

The first build compiles every dependency and takes several minutes; later builds are incremental.

Export from the command line, without the window:

```powershell
cargo run -p ginos-cli -- export my-zine.zine --mode imposed --dpi 300 --out my-zine.pdf
```

`--mode` is `reader` or `imposed`; `--sheet` picks a different sheet size.

## Making a release

Push a version tag and the [Release workflow](.github/workflows/release.yml) builds the Windows installer on GitHub and attaches it to a draft release:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Check the draft on GitHub, then publish it. Raise `version` in `src-tauri/tauri.conf.json` and the `Cargo.toml` workspace before tagging a new version.

## How it is built

| Folder | What it is |
| --- | --- |
| `crates/ginos-core` | The Rust core. It lays out every glyph once; the same positions are drawn on the canvas and written to the PDF, so the screen does not disagree with the print. |
| `crates/ginos-cli` | The command-line front end. |
| `src-tauri` | The desktop shell: joins the interface to the core, and holds the program icons and installer settings. |
| `ui` | The interface (Svelte). It sends intents and draws what the core returns. |
| `fixtures` | The fonts Gino ships with, and the sample zine the tests open. |
