# Index

## The alignment problem

The `feed` pipes through a block of transactions.
These may or may have been pre-filtered on their relevance to Subbit,
and the Provider.

We want to parse each transaction as a collection of Steps, or initializations
of subbit channels.
However, to know what the validator actually validated we need "resolved" inputs.
That is, we need the UTXO of each input, and inparticular, whether it resides at the script address.
Without the payment credential we can do a best guess,
but cannot distinguish an input from a foreign script spent with a recognizable redeemer.

We could assume this won't happen.
However, its a terrible footgun to leave dangling.
Wrong data is worse than making the index fall over.
But even crashing the index risks the potential of lost funds depending on downstream dependency.

The least worst case is to handle unrecognised channels by fallback:
all input channels are declared `Done`, and all Output channels are considered `Init`.

Align inputs and outputs: Address -> Address.
What happens if we do not know an Input address?!

FIXED :: Change the kernel logic to yield on each payment credential not address.
Its slightly more expensive, but it makes less weird the possible txs.
At present you can "cleverly" `Init` before a `Cont`.

The current design does not allow us to align inputs from outputs without knowning the address
of the input.
We can know there must be _atleast_ as many outputs are as there are continuing steps.
We can also align some steps and outputs. For example `Add` step implies `Opened` stage.
Thus we can do slightly better then just requiring alignment.

### Resolving Tx

We have a system of constraints.

Example: Suppose we have three (script) inputs, three steps from the first redeemer
while the other two redeemers are `Defer`, and three outputs.

- If the first input is good, then the other two inputs are good.
  This follows from the validator exhausting the steps,
  there must be at least as many good inputs as there are steps.
- If any input is good, then the first is good. This follows from the following: the good input implies,
  the first is a script input, so this too is good.

- If all steps are `Cont` then the input corresponds to the output.
- If one step is `Eol` then we may or may not be able to align on the basis of step and stage.
  For example an `Add` requires the cont output to of stage `Opened`.

Refined example: Steps are `Eol, Add, Close`.

- If the outputs are in stages `Closed, Opened, Closed`, the we can resolve this.
  The first output is skipped since `Add` requires an `Opened` output.
- If the outputs are in stages `Opened, Closed, Opened`, the we can resolve this.
  The third output is impossible since `Close` requires an `Closed` output.
- If the outputs are in stages `Opened, Opened, Closed`, then we cannot yet resolve this.
- If the outputs are in stages `Opened, Opened, Opened`, then the state is incoherent.
