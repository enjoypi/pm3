# adapters — bidirectional format conversion and Port implementations

Controller / Presenter / Gateway / DTO all live in this layer. No business-rule decisions, no use-case orchestration.

- **XDG defaults cannot be written in yaml**: `substitute_env_vars` does not expand nested defaults, so `${PM3_STATE_DIR:-${XDG_STATE_HOME}/pm3}` leaves the inner form as literal text. The repo's `config.yaml` keeps every root empty (`${PM3_STATE_DIR:-}`) and the derivation lives in `paths.rs`
- A `pm3.home` that is set (env or config) means **legacy single-root mode**: all four XDG variables are ignored and every path stays where it was. That is what keeps the e2e suite and `PM3_HOME=/srv/pm3` installs working unchanged
