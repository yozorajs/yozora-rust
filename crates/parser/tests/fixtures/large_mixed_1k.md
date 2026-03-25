# Parser Large Input Smoke Test

Markdown parsers in production usually process documents that are much longer than tiny examples.
This fixture is intentionally around one kilobyte and mixes several structures so that block and inline
phases both receive realistic pressure. It is not a benchmark sample, it is a correctness sample.

## Network Layer Notes

The transport stack should preserve ordering guarantees while still allowing retries when endpoints
respond with transient failures. A robust implementation records timestamps, request identifiers,
and response hints to help operators investigate behavior under load.

> A quoted line that includes `inline code` and a [reference link][ops].
> Another quote line with escaped markers: \* \_ \` and plain text.

1. First ordered item with **strong** and *emphasis* text.
2. Second ordered item that carries a nested list:
   - child item alpha with ~~delete~~ marker
   - child item beta with ![diagram](https://example.com/diagram.png)
3. Third ordered item with <https://example.com/health> URL.

- [x] task done for telemetry wiring
- [ ] task pending for alert tuning

| component | status | owner |
| :--- | :---: | ---: |
| parser | stable | team-a |
| tokenizer | review | team-b |
| runner | active | team-c |

```ts
export function normalize(input: string): string {
  return input.trim().replace(/\s+/g, " ")
}
```

$$
a^2 + b^2 = c^2
$$

[ops]: https://example.com/runbook "Operations Runbook"
