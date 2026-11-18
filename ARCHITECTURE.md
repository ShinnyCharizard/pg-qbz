# Module boundaries

The executable connects the line-oriented JSON-RPC transport to `Runtime`.
The runtime serializes account changes against catalog requests and report
sending. A shared session context restores local state once and owns the API
client, account controller and playback journal.

`access` represents disconnected, authorizing and connected identities as
separate states. The browser attempt owns cancellation. Callback parsing,
profile resolution, credential discovery and form rendering are independent
modules. Persisted DTOs preserve the account schema used by installed copies.

`catalog` handles browsing and collection operations. `projection` reads the
provider's document shapes and builds host cards. `memo` combines expiration
and per-key reservations; an overlapping read waits for the owner of that key.

`files` stages a JSON transaction in the destination directory, flushes and
syncs it, then atomically replaces the committed snapshot. Reads distinguish
missing, unreadable and invalid state without exposing its contents.

`history` separates persisted records, event transitions and network delivery.
An accepted event commits to an atomic file before acknowledgement. The sender
selects one due envelope and does network I/O outside the ledger lock. Tickets
keep their account, playback context and opaque reporting receipt.

`media-core` performs no I/O. A bounded reader produces track and sample
indexes. Packet indexes validate every span before appending audio and apply
frame transforms directly in the destination allocation. Session and content
keys have distinct types and clear their storage when dropped.

`qobuz-api/media` owns session leases and transfer plans. Each located file
carries the lease that produced its wrapped key. A plan retains its init
metadata, source deadlines and actual quality. Ordered assembly holds at most
three encrypted segments; archive collection uses a bounded unordered window
and restores source order for storage. Dropping either operation drops its
pending network work.

The manifest API version and process protocol remain 1. Package versions start
at 1.0.0. Stable IDs, references and persisted field names belong to the host
and storage contracts and are independent of internal naming conventions.
