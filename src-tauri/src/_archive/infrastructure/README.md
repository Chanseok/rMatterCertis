# Archived infrastructure modules

This folder contains legacy infrastructure modules that are no longer part of the active build.
They were moved here to avoid confusion and enforce the unified HttpClient path (`simple_http_client`).

Archived on 2025-08-20:
- `http_client.rs` (legacy/compat facade around the new client). Use `crate::infrastructure::HttpClient`
  which re-exports the unified client from `simple_http_client`.

Reason:
- We standardized on a single HttpClient with a global token-bucket rate limiter and config-driven RPS.
- Keeping the legacy facade in-tree caused ambiguity around `new()` vs `create_from_global_config()`.

If you need to reference the old implementation for historical reasons, see `http_client.rs` in this folder.
