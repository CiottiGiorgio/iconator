# Exposing Core Icon Fetching Logic Through a Web Server

## Problem

We have code that maps file names and folder names to integer IDs representing SVG files.
The pipeline starts with three lists of tuples that act as the mapping from textual information to the target ID.
One for file basenames, one for file extensions, and finally one for folder names.
Each list of tuples is then built (serialized) as a `fst::Map`-compatible bytearray and written to disk.
The current library loads (deserializes) this bytearray as a `fst::Map` with unsafe code and exposes two functions that implement the querying logic.
The "deserialization" skips error checking because the code implicitly trusts the build system to have generated a correct bytearray.

For filenames, the basename is queried first.
If that comes up empty, then the extension name is queried.
For folder names, an exact match is queried.

We want to expose this existing code through an HTTP web server.


## Notable Features of the Current Codebase & Problem

1. `fst::Map`-compatible bytes are embedded in the binary read-only section.
2. The `fst::Map` wrapper over the binary data is loaded (lazily) into static memory.
3. `.unwrap()` usage in the query code paths which could fail when a match does not exist.
4. The mapping doesn't offer functionality to add/remove entries.


## Assumptions

1. We will assume that the server will implement an equivalent logic for matching a query without using `.unwrap()` in fallible code paths.
2. We will assume that the server shall offer a way to query for an SVG read from disk, in addition to querying for its ID.
3. We will assume that the SVG set will remain small (see Caching in Solution).
4. We will assume that the web server does not offer any functionality to change this mapping.
5. We will assume that the server is meant to run on a UNIX-compatible system (see Productionize the Web Server).

## Solution

This document proposes using Axum as the web server implementing this job.

### Caching

This document also proposes caching strategies for the SVGs.

At the current size of the set, a server-side cache is not strictly necessary.
The operating system's page cache will likely keep recently read SVG files in memory without frequent eviction, so repeated disk reads are cheap.
We still choose to implement a server-side cache, so that the server is already set up for success if the set of SVGs grows beyond what the page cache can comfortably hold.

The server-side cache is a memory-bounded in-memory cache of SVG contents keyed by ID, implemented with [`moka`](https://crates.io/crates/moka) (`moka::future::Cache`) alongside the Axum server.
The cache is bounded by total bytes, not by entry count, using a weigher, because SVG sizes vary widely.
On a miss, the handler reads the SVG from disk, computes its ETag (a BLAKE3 digest), and inserts both into the cache.
The ETag is a digest of the SVG computed on a miss, not at build time. This keeps the build simple at a small runtime cost, and versioning the content (see Productionize) covers the same ground.

In addition, we use headers to inform the client that this content will not change (Cache-Control and ETag).
The client needs to understand these headers and cache SVGs locally.


## Productionize the Web Server

The Solution section is the scope of the current implementation.
Everything below is a future goal, not a commitment.

1. Package the web server into a docker image so that our assumption of running on a UNIX-compatible system is respected.
2. Version the mapping and the SVGs together. The mapping is compiled into the binary, so the SVGs should ship in the same image.
3. Include the version in the URL or the ETag, so that cached SVGs don't go stale across releases.
4. Replicate the web server for horizontal scaling. Each instance has its own cache, which is fine for immutable content.
5. Put a CDN in front of the server if latency or bandwidth requires it. The Cache-Control and ETag headers already make this possible without code changes.
6. Observe how the client-side and server-side caches are doing. Add probes for the number of responses without a body (client cache hits) and the number of requests that hit the server-side cache. This tells us if an assumption about the SVGs is not being honoured.
