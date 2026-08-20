# Subbit-rs

A rust re-write of the subbit stack.

## TODOs

- [ ] rationalize configs (use figment)
- [ ] extend echo client to use issuer.
- [ ] have a naive tx submitter.
- [ ] documentation outlining the stack.
- [ ] rationalization of core.

## Rationalization of core

What we require is that components that talk over the wire share wire definitions.

Nothing builds a `Datum` without also submitting a tx?
So `Datum` can appear in `subbit-tx`?

The server was multiple APIs:

- `x` executable `o`: used by issuer
- `a` admin: used by index and tx-submitter.

A more optimal crate arrangement?

```
tx
l1 - sessions? / cli
l2-wire - cli?
issuer
server
client
backing-wire
index
tx-service
conformance
integration
```
