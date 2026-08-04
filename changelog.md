# changelog

## **2026-08-04** - 0.0.4

1. REMOVED dead code from the cli download command handler (the download runner never returns without output).
1. MODIFIED cli progress bars to stay hidden under cargo test, keeping the test output clean.
1. ADDED initial presets for guyana, paraguay, peru, suriname, uruguay, venezuela, netherlands and switzerland.
1. ADDED more tests to cli mod to cover all command handlers, clap parsing and exit paths.

## **2026-08-02** - 0.0.3

1. ADDED initial presets for chile, colombia and ecuador.
1. ADDED user-agent to http requests.
1. ADDED more tests to extract mod.

## **2026-07-11** - 0.0.2

1. REMOVED .test files from sonar analysis to reduce noise.
1. MODIFIED http-server files to fix sonar security issues.
1. MODIFIED cargo.lock to fix audit report.
1. ADDED new tests to osm_pbf_file mod to improve coverage.
1. ADDED initial presets for portugal, argentina and bolivia.
1. ADDED signal handler for shutdown (ctrl + c).

## **2026-06-28** - 0.0.1

1. first public version :)