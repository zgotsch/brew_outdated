# brew_outdated

`brew_outdated` is a utility for keeping your `brew`-installed executables up-to-date. It examines your shell history and looks for executables which have been recently used and are out-of-date according to `brew outdated`. It will only detect executables which were installed by `brew`.

Supports `bash`, `zsh`, `fish`, and `nu`.

Does not currently support finding out-of-date executables which were installed by `brew cask`.

`brew_outdated` runs `brew update` in the background to update `brew` and the brew formulae, so the suggestions for updates stay accurate. If this update fails, `brew outdated` will display a message next time it is run.

`brew_outdated` is about as fast as the `brew outdated` it relies on, so is recommended to run it in your shell's startup file.

## Installation

To install, run

```
cargo install brew_outdated
```

## Usage

To run, ensure that `~/.cargo/bin` is in your `PATH`, and run with

```
brew_outdated
```

If you would like to run it each time you start a new shell, you can add it to your shell's startup configuration file. Here are the locations of startup configuration files for the supported shells:

| shell  | default config location                                                                                                                        |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `bash` | `~/.bashrc`                                                                                                                                    |
| `zsh`  | `~/.zshrc`                                                                                                                                     |
| `fish` | `~/.config/fish/config.fish`                                                                                                                   |
| `nu`   | can be configured under the startup config option: https://www.nushell.sh/blog/2020/04/21/nushell_0_13_0.html#startup-commands-jonathandturner |
