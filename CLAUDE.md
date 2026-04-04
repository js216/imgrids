- Snapshot tests: before refactoring rendering code, run
  `SNAPSHOT_UPDATE=1 cargo test -p snapshot-tests` to generate golden
  PPM files for all demo menus. After making changes, run
  `cargo test -p snapshot-tests` to verify every menu still renders
  identically. Golden files are gitignored and local-only.
