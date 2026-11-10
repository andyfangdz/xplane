# xplane-gauge-xml

Owned XML trees and presentation-template tokens for simulator gauge loaders.
The crate has no aircraft definitions, simulator bindings or RPN execution
policy. XML parsing uses the pinned `roxmltree` dependency rather than a custom
XML grammar.

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

Run synthetic input/template tests with `cargo test -p xplane-gauge-xml`.
