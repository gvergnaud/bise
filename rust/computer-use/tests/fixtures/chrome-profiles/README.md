A Chrome user data dir's files as profiles.rs reads them, copied from two
runs of a throwaway headless Chrome 157 (computer-use/extension/test/harness.ts:
`--user-data-dir` in $TMPDIR, the extension loaded, its hello said):

- `Default/Local Extension Settings/<id>/`: run 1's extension storage, which
  holds the nonce of `nonce-default.txt`; `Local State`: run 1's (its profile
  cache names `Default` "Your Chrome");
- `Profile 1/Local Extension Settings/<id>/`: run 2's storage (nonce in
  `nonce-profile1.txt`), absent from `Local State`.

No real profile's data: both profiles were made for this and deleted.
