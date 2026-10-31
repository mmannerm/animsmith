# Evaluation model V3

`urn:animsmith:skill:animation-pack-evaluation:3` is the immutable evaluation
contract for `urn:animsmith:schema:collection-output:12`, with output-v19 and
measurements-v18 nested evidence. It retains the V2 vocabulary, canonical JSON
owner and relationship checks. V2 remains bound to V11; the current writer and
workflow remain V11/V2 during this reader-preparation step.

V12 explicitly retains a source whose analysis exceeded sampling admission as
`result: {"state":"unavailable","reason":"sampling_limit_exceeded"}`. Its input,
config, digest, loader and dependency closure remain authoritative. Copy that
source state into the model binding; retain its logical rows as not evaluated
with unavailable measurements. Do not turn unavailable measurements into zero,
not-applicable values, or evidence of quality. Other assets retain their measured
values. The whole model cannot claim complete evidence when the collection is
incomplete.

Validate and render an existing V12-bound model with the checkout-matched binary:

```sh
python3 .agents/skills/evaluate-animation-packs/scripts/validate_evaluation_model_v3.py model.json --binding collection-v12.json --animsmith /path/to/animsmith --check-canonical
python3 .agents/skills/evaluate-animation-packs/scripts/render_evaluation_model.py model.json --binding collection-v12.json --animsmith /path/to/animsmith --report report.md --appendix report-evidence.md
python3 .agents/skills/evaluate-animation-packs/scripts/validate_report.py report.md --appendix report-evidence.md --evaluation-model-v3 --report-format 2
```

The validator captures one bounded no-follow regular file, submits those exact
bytes to the strict Rust reader and requires the exact V12 handshake. The same
buffer supplies JSON and byte identity; the path is not reread after validation.
The renderer dispatches only on the explicit model URN. V2 and V3 use the same
implementation with immutable revision pins, not parallel semantic readers.
