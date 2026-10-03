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
3. We will assume that, for this single-instance web server, a memory-bounded in-memory cache of SVGs is reasonable.
4. We will assume that the web server does not offer any functionality to change this mapping.
5. We will assume that the server is meant to run on a UNIX-compatible system (see deployment).

## Solution

This document proposes using Axum as the web server implementing this job.

It also proposes to use a cache for the SVGs.
It will achieve 
