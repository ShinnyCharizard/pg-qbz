# Qobuz source

A native process source for browsing the catalogue, managing playlists and
playing music. Browser authorisation keeps account credentials in the
plugin's private data directory.

## Runtime directories

The worker waits for `initialize` before opening account state or starting its
report delivery task. `data_dir` and `cache_dir` must both be absolute paths
provided by the host. Repeated initialization may update locale and event
negotiation with the same directories. Directory changes require a restart.
Each platform archive works with both native and Flatpak hosts that implement
this directory contract.

## Account

Fetch application credentials or enter them manually, then authorise
through the browser. Each secret field has its own visibility control.
Reopening the form or starting a browser attempt hides the fields.
Authorisation uses a fresh loopback path and expires after five minutes.

Credentials and sessions are stored in `account.json`. Writes are atomic and
private on Unix; masking is not encryption at rest. Account password login,
delegated sessions and Connect are not implemented.

## Catalog and playback

The source provides personal recommendations, charts, genre browsing, mixes, artist and
label navigation, lyrics, library metadata and playlist editing. Repeated
playlist tracks keep separate membership IDs.

Track display artists use structured performer roles when available. Technical
credits remain in their own metadata. Canonical display names take precedence over legacy casing. Without one,
multiword uppercase names use a conservative casing fallback; single-word
acronyms and existing mixed case remain. The adapter pairs the primary name
with its ID and omits unrelated album artist IDs.

Playback resolution carries quality and ReplayGain metadata. The media helpers
handle segmented CMAF transfers, ordered FLAC assembly and encrypted archives.
Playback reports use measured audible time from player events. Resolving or
preloading a track does not count as listening. Signing out clears the outbox.

## Library and magazine

When supported by the host, Library expands into All, Playlists, Tracks,
Releases, Artists, Labels and Awards. All mixes the explicitly saved entities;
artist and label views also include those attached to saved releases or tracks.
Filters include membership, hidden purchases, high-resolution content, genres,
search and playlist ownership. Filters and ordering remain independent by page.
All also exposes the same navigation tabs in its header on supporting hosts.

An account-scoped SQLite index supplies bounded pages and keeps its previous
snapshot readable during synchronization. Reads do not download a complete
library into the interface. Manual refresh also updates parent metadata.

Labels fetch their own artwork in batches for the visible page and keep up to
512 cached URLs per account. Album artwork is never substituted for a label.
Artist, label and award cards carry optional Lucide fallback identifiers alongside
their images. Supporting hosts draw these locally when no usable image is available.

Awards lists the complete available catalogue, with a separate favourites/following
filter. Its count follows the current search and filter. The account's index
caches at most 2048 award entries in 1 MiB, refreshed on demand after one day
or by manual refresh. Catalogue entries do not become saved Library items.

Magazine supplies native article lists and plain-text reading pages with a few
images. The account selects its editorial region; public feeds receive no
account credentials. Scripts, embedded players and page layouts are omitted.
Up to 16 pages remain cached per account for offline reading, with a 1 MiB
limit per document. Articles contain at most 256 blocks, 512 KiB of text and
eight images including the cover. No embedded browser is used.
Editorial images use public server derivatives of at most 1600 pixels on their
longest side; list thumbnails use 600 pixels. Original full-size images are
not passed to the reader.

These views use optional feature negotiation during initialization. Magazine
requires native article support; it is absent on older hosts. Library falls
back to its existing tabs and catalog pages when needed. Home keeps its shelves.

## Build and checks

Build with `python3 tools/build-native`. On Linux, set `PLUGIN_BUILD_LOCK`
to the workstation's shared Rust build lock. The archive contains the native
executable, manifest, icon and license notices. Build separately for each
operating system and architecture.

Run `python3 tools/verify` with that shared build lock configured and check the packaged
worker with `python3 tools/smoke /path/to/plugin`. The smoke uses a disposable
profile. Optional public credential discovery uses `--fetch`; it does not sign
in. Tests cover account state, callbacks, catalogue projection, playback events,
process framing, media bounds and cancellation.

## macOS

Build and verify a separate package on macOS. Linux archives cannot run there;
Linux test results do not establish macOS coverage for a new revision.

## Licence

Distributed under GPL-3.0-or-later. See `COPYING`, `NOTICE` and the
MIT licences in `crates/` for the upstream API, model and media code.
