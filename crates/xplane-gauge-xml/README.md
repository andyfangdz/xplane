# xplane-gauge-xml

Reusable XML gauge definitions, calculator bindings and presentation interfaces.
The crate owns XML parsing, macro/key/click definitions, inherited visibility,
artwork transforms, ordered source registration and template linking/rendering.
It has no aircraft definitions, assets, simulator bindings or numeric policy.
XML parsing uses the pinned `roxmltree` dependency.

`Document::parse` reads UTF-8 bytes, including a UTF-8 BOM, and owns decoded
element names, attributes and direct text. Element IDs are preorder ordinals;
comments and text do not receive IDs. Direct text follows ElementTree semantics:
text before the first child element is retained, and child tail text is not.
Entities, CDATA and XML newline normalization are handled by the XML parser.
Namespace names use `{uri}local` spelling. Inputs are bounded to 32 MiB and two
million XML nodes, with DTDs and external entity resolution disabled.

`child`, `find` and `text` traverse direct children. `find` accepts slash-separated
tag names, not XPath syntax. `descendants` includes the starting element, as does
`named` when its tag matches. Documents and nodes can be cloned and retained
independently of their original input buffers.

`template::parse` reads gauge-string percent escapes, nested calculator
expressions, conditional markers, labels and the selected numeric-format subset.
It retains calculator text without parsing or executing it. Expression offsets
count Unicode scalar values in the original template. A runtime can connect
those tokens to its own RPN parser and presentation layer.

```rust
use xplane_gauge_xml::Document;
let document = Document::parse(b"<Gauge><Value>1 &gt; 0</Value></Gauge>").unwrap();
assert_eq!(document.text(document.root(), "Value"), Some("1 > 0"));
```

`definition` reads direct or nested macro definitions, ordered key handlers,
mouse actions and SDK click masks. Macro spelling is an explicit `MacroStyle`;
legacy click aliases and ignored tokens remain host policy. Visibility conditions
retain root-to-leaf order and calculator-expression boundaries.

`geometry` composes ancestor positions, bounded shifts, clipping rectangles and
image axes into calculator source expressions. It rejects unsupported rotation.
It also reads BMP/PNG header dimensions without decoding the artwork. The host
selects files, assets, source-location names and image presentation policy.

`programs::Programs` registers ordinary source text, stage labels and macro
snapshots in order. IDs are one-based ordinals; equal expressions remain distinct
so their source diagnostics and bindings are retained. No hashes, serialization
format or precompiled instruction artifacts are required by this layer.

`template::link` connects calculator tokens to host program IDs through a
registration callback. `presentation` supplies canvas, image and readout types,
plus conditional rendering. Rendering invokes the evaluator and numeric-format
callbacks only in active branches; text encoding, exact digit formatting, VM
state, floating-point mode, scheduling and simulator calls belong to the host.
Output is bounded to 4095 bytes and nesting to 256 levels.

The reusable pipeline is **XML → definitions/source registration → RPN parser →
VM → presentation callbacks**. A host can use `xplane-rpn` for parsing/execution;
it is a test-only dependency here, so consumers may choose another evaluator.
The synthetic integration test exercises that complete pipeline without any
aircraft input. This is the supported gauge subset, not every P3D XML feature.

Run synthetic definition, geometry, template and VM integration tests with
`cargo test -p xplane-gauge-xml`.
