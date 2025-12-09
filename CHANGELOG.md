# v0.0.12
## Fixes
Fixed issue where file system events from the build step polluted the watcher leading to .model.json files being immediately deleted after being created in certain scenarios.

# v0.0.11
## Fixes
Derives are now resolved properly when there is no luaurc file.

# v0.0.10
## Fixes
Removed redundant print statement.

# v0.0.9
## Changes
- Adds support for Luaurc aliases in derives. It attempts to find a Luaurc file either at the input directory or the parent of the input directory. You can also manually specify a Luaurc file via the `luaurc` flag.
- Throws an error if the input doesn't exist or isn't a directory.

## Fixes
- Fixed issue where renaming a directory did not update the corresponding `.model.json` files.
- Fixed issue where deleted `rsml` files weren't removed from the internal dependencies map.

# v0.0.8
## Fixes
- Fixed issue where the input directory was cleaned (the removal of redundant .model.json files) on initialisation instead of the output directory.

# v0.0.7
## Fixes
- Fixed issue where moving a .rsml file to another location would not remove the corresponding .model.json
- Fixed issue where redundant rsml .model.json files weren't deleted on initialization.

## Changes
- Added a `build` command.


# v0.0.6
## Fixes
- Fixed issue where derives would not be properly resolved.


# v0.0.5
## Fixes
- Fixed issue where macros from indirect derives were not being imported.

## Changes
- Removed some dead dependencies


# v0.0.4
## Changes
- The `run` command has been renamed to `watch`.
- added a `version` command.
