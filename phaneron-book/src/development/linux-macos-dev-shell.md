# MacOS / Linux (Dev Shell)

Note: The following has only been tested on Linux but should work on MacOS.

The most consistent way to produce builds of Phaneron on Linux is to use a combination of [nix-shell](https://nixos.wiki/wiki/Development_environment_with_nix-shell) and [direnv](https://direnv.net/).

Start by installing [nix](https://nixos.org/) with `sh <(curl -L https://nixos.org/nix/install) --daemon` then create `~/.config/nix/nix.conf` with the following contents:

```ini
experimental-features = nix-command flakes
max-jobs = auto
```

Next install direnv:

```bash
nix profile install nixpkgs#direnv
```

And add the extension `mkhl.direnv` to VS Code.

Next run `echo 'eval "$(direnv hook bash)"' >> ~/.bashrc` then enter the root directory of the Phaneron repo and run `direnv allow`.

If everything has worked, you should be able to run `cargo build` as usual to produce a build and VS Code should provide you with syntax highlighting and code formatting as you would normally expect.

The line added to your `.bashrc` file means that you will automatically enter a dev shell built from `flake.nix` when you change directory into the Phaneron folder. This will also apply to your shell in VS Code.

## Limitations

Currently this is only useful for producing builds of Phaneron, it cannot be used as a method for running Phaneron as `flake.nix` is missing the requisite magic to find the right OpenCL drivers for your system, this means that Phaneron will fail to create an OpenCL context and will panic on the error `ClError(-1001)`. This will hopefully be fixed in the future.

## QoL improvements

If you intend to contribute to Phaneron and modify the `flake.nix` file then install support for `.nix` file formatting and syntax highlighting:

```bash
nix profile install nixpkgs#nixpkgs-fmt nixpkgs#rnix-lsp
```

Then add to your VS Code settings:

```json
"[nix]": {
    "editor.formatOnSave": true,
    "editor.defaultFormatter": "jnoortheen.nix-ide"
}
```

And install the extension `jnoortheen.nix-ide` in VS Code.
