# Navigation data records

Byte-preserving radio-record parsing and ordered layer merging, without an
X-Plane SDK dependency. The reader accepts 1150/1200 headers, checks record
fields and runway associations, and reports duplicate identities. Later layers
replace an identity at its original position.

`parse` takes a `Numbers` implementation so callers can preserve their native
numeric-extraction contract. `MsvcNumbers` uses classic-locale MSVC stream
semantics on Windows, including numeric prefixes and floating-point status.
Textual fields and diagnostics retain their original bytes.

File selection, optional-layer policy and simulator integration belong to the
caller. This crate reads navigation metadata; it does not infer facilities from
instrument procedure files or implement a navigation receiver.
