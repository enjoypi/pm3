# usecases — application business rules

Interactors + Output Ports (traits). Interaction with outer layers goes only through the traits under `ports/`; implementations live in `adapters`, injection in `frameworks`.

- Global environment values (`EnvScope::Global`) are filtered out of the fingerprint (`sorted_env`), `Injected` (HOME) and `App` are kept: editing the shared `pm3.env` must not evict and respawn every service. The consequence to expect — a global and an app declaring the **same key** drops the app entry, so the fingerprint loses one line and the service restarts once. That is correct (the effective value changed source) and surprising
