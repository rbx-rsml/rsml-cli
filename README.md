# RSML CLI

A CLI for RSML which converts `.rsml` files to `.model.json` files.

- - -

# Setup

1. Add the CLI to your rokit config.
```toml
rsml = "rbx-rsml/rsml-cli@0.0.13"
```

# Watching
Use the `watch` command to continuously hot-reload `.rsml` files from an input directory into `.model.json` files in an output directory.

```
rsml watch <project_path>
// rsml watch /src
```

You can optionally define a different output directory and luaurc file path.
```
rsml watch <project_path> --output <output_path> --luaurc <luaurc_path>
// rsml watch /src --output /dist --luaurc /configs/.luaurc
```

# Building
Use the `build` command to sync `.rsml` files from an input directory into `.model.json` files in an output directory.

```
rsml build <project_path>
// rsml build /src
```

You can optionally define a different output directory and luaurc file path.
```
rsml build <project_path> --output <output_path> --luaurc <luaurc_path>
// rsml build /src --output /dist --luaurc /configs/.luaurc
```
