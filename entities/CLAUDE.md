# entities — enterprise business rules

No I/O, no async, no framework dependencies (except the `serde`/`thiserror` derives).

- `mask_secret` lives here, not in the presenter: "pm3 never transmits a credential in the clear" is a business safety invariant, and the masking has to happen before the value leaves `usecases` — otherwise `describe --json` prints the plaintext and a refused request logs it into `resp`. Length is reported in **characters**, the same unit the truncation uses
