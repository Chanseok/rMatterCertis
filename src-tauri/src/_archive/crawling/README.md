This folder contains the deprecated CrawlingEngine-based implementation.

Status:
- Archived on 2025-08-16 during cleanup/phase-1.
- Replaced by actor-based crawling (Session/Batch/Stage Actors) and a global Sqlite pool.

Notes:
- The former in-memory Sqlite path was only intended for dev/test and is not suitable for production or observability.
- If you need a temporary database for tests, prefer a temporary file-based sqlite initialized via the same global pool initializer.
