# Sources

Initial source registry is defined in `sources.yaml`.

Implemented in current prompt sequence (initial chunk completed: 5 sources):

- `appen-crowdgen` (`PublicHtml`)
- `clickworker` (`PublicHtml`)
- `oneforma-jobs` (`PublicHtml`)
- `telus-ai-community` (`PublicHtml`, public pages only)
- `prolific` (`ManualOnly`, manual ingestion fixtures)

Chunking note:
- PROMPT_10 requires expansion in chunks of 3-5 sources.
- Current `sources.yaml` contains 5 sources and is implemented as a single chunk.
- Future additions should be appended to `sources.yaml` and implemented in subsequent 3-5 source chunks with tests between chunks.

## Generator Example (Not an Enabled Source)

`sample-source` is a scaffold-generation example created to verify `rhof-cli new-adapter`.
It is not listed in `sources.yaml`, is not enabled in sync, and should not be treated as a supported source implementation.
