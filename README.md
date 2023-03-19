# Phaneron (Rust Edition)

## Getting Started

1. See the [Developer Requirements](#developer-requirements) section for dependencies etc.
2. Rename `video_inputs.example.json` to `video_inputs.json` and modify its contents to point to some videos that you want to play. If you want audio, the first file in this list should contain an audio track.
3. Run the command `cargo run`.
4. Start up the [Phaneron Demo App](https://github.com/superflytv/phaneron-demo-app).

Note: Phaneron will attempt to bind to both port 8080 and 9091. This will be configurable in the future and will be reduced to a single port.

## Developer Requirements

### Linux
- `rust` (see [rustup](https://rustup.rs/))
- opencl development headers for your system (this may vary based on your hardware configuration)
- `libvpx`
- `libclang-dev`
- The `libav*` family of libraries

### Windows
- TBA

### Mac
- TBA

## License

No license is currently attached to this work as it is very much a work in progress. However, much of this work has been heavily inspired by work by [Streampunk Media Ltd.](https://github.com/Streampunk/phaneron) which is released under the [GPLv3 license](https://github.com/Streampunk/phaneron/blob/master/LICENSE). As such, some files within this project have been marked with GPLv3 attribution headers. This project will soon move to a plugin model and will use a non-GPL license for some aspects of the plugin architecture to permit the publication of proprietary plugins. All other areas of the code will be licensed under GPLv3.
