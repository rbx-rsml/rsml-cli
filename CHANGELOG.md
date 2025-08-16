# v0.0.8
## Fixes
- Fixed issue where the input directory was cleaned (the removal of redundant rsml .model.json files) on initialisation instead of the output directory.

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