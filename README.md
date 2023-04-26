# Phaneron (Rust Edition)

## Getting Started
1. See the [Developer Requirements](#developer-requirements) section for dependencies etc.
2. Rename `video_inputs.example.json` to `video_inputs.json` and modify its contents to point to some videos that you want to play. If you want audio, the first file in this list should contain an audio track.
3. Run the command `DEVELOP_PLUGINS=true cargo run`.
4. Start up the [Phaneron Demo App](https://github.com/superflytv/phaneron-demo-app).

Note: Phaneron will attempt to bind to both port 8080 and 9091. This will be configurable in the future and will be reduced to a single port.

## Configuration
- Set the `DEVELOP_PLUGINS` environment variable to load plugins from the `target/` directory. This allows you to edit plugins and run Phaneron without having to separately build each plugin and copy it to the plugins folder.
- If `DEVELOP_PLUGINS` is set, `PLUGINS_CFG_FILE` can be used to point to a different `plugins.toml` file.
- If `DEVELOP_PLUGINS` is not set, `PLUGINS_DIRECTORY` can set to change the location of the plugins folder (default is `plugins` in pwd).
- `RUST_LIB_BACKTRACE` can be set to obtain a backtrace from dependencies. This is enabled by default for debug builds.
- `RUST_LOG` can be specified to obtain logs from crates. By default this is set to `phaneron=info` which will include info-level logs for Phaneron and all loaded plugins.

For debug builds the use of a `.env` file is supported. This file is not loaded for release builds.

## Repository Structure
- `phaneron/` contains Phaneron itself in both library and binary formats. This is the target for `cargo run` within this workspace.
- `phaneron-plugin/` is a library that provides type interfaces to help with developing plugins for Phaneron in rust.
- `phaneron-plugin-utils/` is a library that contains utilities that are useful for plugins.
- `phaneron-plugin-*` are plugin crates.

## Conditionally Building Plugins
Some plugins may not be supported on all platforms. Currently opting out of building a plugin is a very manual process, this may improve in the future. First, edit `Cargo.toml` and remove any plugins that you don't want to build from `default-members`. Then edit `plugins.toml` and remove the plugins that you are not going to be building in order to prevent Phaneron from trying to load those plugins.

## Adding a new Plugin
1. Add an entry to both `members` and `default-members` in `Cargo.toml` for the new plugin.
2. Add an entry to `plugins.toml` for you plugin (this should match the value in `package.name` in the `Cargo.toml` file for your plugin).
3. Run `cargo new --lib phaneron-plugin-my-plugin-name`.
4. Refer to existing plugins for example code.

## Developer Requirements

This section covers requirements for `phaneron`, `phaneron-plugin` and `phaneron-plugin-utils`. Refer to the documentation for individual plugins for their respective dependencies.

### Linux
- `rust` (see [rustup](https://rustup.rs/))
- opencl development headers for your system (this may vary based on your hardware configuration)
- `libclang-dev`

### Windows
- TBA

### Mac
- TBA

## License
Refer to individual crates for their licenses. Most work is licensed under GPLv3, with the exception of the `phaneron-plugin` crate which is licensed under MIT. This is to allow for the publication of proprietary plugins.

## Acknowledgments
Much of this work has been heavily inspired by work by [Streampunk Media Ltd.](https://github.com/Streampunk/phaneron)
