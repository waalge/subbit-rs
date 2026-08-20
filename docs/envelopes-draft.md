# Envelopes

Subbit works with provider-consumer relations.
Subbit works most immediately in contexts with consumer driven events,
but can accommodate contexts that require, say, time based billing, if projected onto consumer driven events.

Messages are being passed between consumer and provider: the consumer requests service, and the provider responds to those requests.
Subbit **envelope**s are bits of data that are attached to these requests and responses to facilitate the operation of subbit.
We wish to specify the enevolopes.

## Considerations

Four loosely coupled dimensions of variation were identified regarding the interaction between consumer and provider.
Each may augment what envelops look like.

### Interaction shape: discrete vs. session

A discrete exchange, such as a single HTTP/1.1 request/response, can carry a full proof with every request.
A long-lived session, such as a WebSocket connection, needs a way to amortize proof cost across many frames instead of re-proving on each one.

### Cost knowledge: priced vs. metered

A priced request has its cost known on request, so it can be gated before service is rendered.
A metered request's cost is only known after the fact, which means the claim has to be reconciled after usage is known rather than checked purely up front.
This changes where in the flow verification happens.

### Execution concurrency: sync vs. async

In low-latency, high-ingress applications, intercepting and re-verifying every single frame inline isn't affordable.
Billing has to be manageable asynchronously alongside request handling, out of the hot path, rather than forcing sync verification of every frame.

### Auth structure: iou vs. free

A message that raises the balance carries a fresh `Iou` and is (by definition) iou-bearing.
It contains a (new) signature that proves account control.
Free messages are the common case in a session and a potentially frequent occurrence in the case of metered requests.
These are required for auth where no Iou is needed.
In addition, if the auth for free messages is cheap, then it can also be employed as a spam filter even on iou-bearing messages.

## Solution(s)

There is not a one-size-fits-all solution.  
We propose solutions for some cases, providing components and inspiration for others.
For example, sessions can use an envelope suitable for a discrete usecase for initiation,
and then use IOUs in mid session payements.

### Iou Only

In discrete, priced contexts where all requests require a new IOU (ie no free requests) no additional auth is required.
Thus, the envelope can take the following form:

```rust
pub struct IouOnlyRequest {
    account : Account,
    iou : Iou,
}
```

A server may not need to respond with any subbit data:
the service provision may suffice the client - it has been served.

Otherwise the server may respond with

```rust
pub enum IouOnlyResponse {
    Ok { available: u64 },
    Stale { iou: Iou, available: u64, },
    Ko { status : u8, msg : String },
}
```

`available` is the amount the provider assesses as redeemable.
`Stale` responds to an IOU that is not later than the previous. This is helpful when the client has forgotten state,
and can verify latest from their own signature.
`Ko` says "Something else has gone wrong".

### Free Auth : HMAC

HMAC is the obvious best fit: incredibly cheap to verify, essentially stateless per-message.
It requires some kind of registration (a handshake to establish the shared key),
but this can be inlined into the exchange that also establishes the session or Iou.

In order to get a `mac`, the client must first demonstrate proof of possession of key.
This could be an IOU, as long as it is latest.
Alternatively it can be arbitrary body provided that it is proof of possession,
and cannot be used between accounts (ie is specific to the tag).
The alternative is the preferred solution.

Since the signing key is already used for IOUs, the To-Be-Signed bytes used in PoP must not clash with an IOU.
We propose the following:

```rust
const DTS : &[u8] = b"SUBBIT_AUTH";

#[derive(Encode)]
pub struct Tbs<Body> {
    #[n(0)]
    dts: Vec<u8>,
    #[n(1)]
    body: Body,
}

pub fn tbs(b : impl Encode) -> Vec<u8> {
    to_vec(Tbs { dts: DTS.into(), body })
}
```

The service providers terms of service should include in their auth policy the shape of the `Body` and the `DTS`.
The body should contain the account information. We propose a "Standard" that will fit unspecialized cases.

```rust
pub struct Body {
    #[n(0)]
    key: VerifyingKey,
    #[n(1)]
    tag: Tag,
    #[n(2)]
    ttl: Duration,
}
```

The `ttl` is abosulte posix time after which the token is invalid.
Specialized cases may wish to include different, or additional data from the client.

A "Standard" mac request is then:

```rust
pub struct MacRequest {
    body: Body,
    sig: ed25519::Signature, // Sig of the corresponding TBS
}
```

On receiving the request, the server verifies the signature, and verifies the account exists and is serviceable.
On success, the server responds with a token.

```rust
pub struct Token {
    body: TokenBody,
    sig: mac::Signature,
}
```

The token may or may not be readable to the client.
The expectation can be specified in the terms of service.

For inlined registration we can wrap as:

```rust
pub enum Auth {
    #[n(0)]
    Pop(#[n(0)] MacRequest),
    #[n(1)]
    Mac(#[n(0)] Token),
}
```

Note: It is implicit what the mac scheme here is.
We prepose prekeyed blake3 with mac signature length of 20 as a sensible default.

An error will communicate "new token required".

### Standard

We propose the "standard" envelope:

```rust
pub struct StandardRequest {
    auth: Auth,
    iou: Option<Iou>,
}
```

This accommodates the metered case and priced case where there are also "free" requests (no IOU required).

"Standard" should also pick a fixed mac signature length: 20 bytes.

### Headers

The protocol proposes but does not dictate the specificaiton.
There are a few proposed solutions for envelopes via headers.
The "right" solution depends on the context of integration.
Intstances should advertize their header policy as part of their terms of service.

Implementers are discouraged from deviating from the proposed solution,
unless it is advantageous.

On size: the full "Standard" header should be < 250 bytes.
Thus, of no concern.

#### Variants & Versions

Given we have a variety of envelopes, it may be advantageous to specify the Variant and version.

Variants can be optional or required. Variants:

- `Iou` - for Iou Only envelope
- `Std` - for the standard envelope

Version can be optional or required. The current version is `1`.

Note: No trailing hyphens, eg `Subbit-Iou` never `Subbit-Iou-`.
Parsers may or may not be permissive.

To add a variant, submit a PR.

#### Authorization

The preferred solution is to use `Authorization` header.

```sample
Authorization: Subbit-<Variant>-<Version> <Envelope-Base64>
```

If the variant and version are disambiguated via the Terms of Service policy, then
the following should also be accepted.

```sample
Autorization: Subbit <Envelope-Base64>
```

This preferred on the gounds that:

- It is semantically honest - the envelope carries auth.
- The standard accommodates custom protocols.
- `Authorization` is more often excluded from logging of reverse proxies, which may be desirable.

Responding with `Authorization` header is not in standard, although not prohibited.

#### Custom

If authorization is unavailable or deemed undesirable, then use the `Subbit` header.

- Use `Subbit: <Variant>-<Version> <Envelope-Base64>` or
- Use `Subbit: <Envelope-Base64>`

This should be used also for responses.
