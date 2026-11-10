use xplane_gauge_xml::{
    Document, Error,
    template::{self, Kind},
};

#[test]
fn owns_normalized_entity_cdata_text_and_element_preorder() {
    let document = {
        let input = b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?><Gauge Name=\"A &amp; B\">head\r\n<!--ignored-->more<![CDATA[ & raw\r\n]]><Element id=\"first\"><Value>1 &gt; 0</Value></Element>tail<Element id=\"second\"/></Gauge>";
        Document::parse(input).unwrap()
    };
    assert_eq!(document.root(), 0);
    assert_eq!(
        document
            .nodes()
            .iter()
            .map(|n| n.tag.as_str())
            .collect::<Vec<_>>(),
        vec!["Gauge", "Element", "Value", "Element"]
    );
    assert_eq!(document.node(0).attribute("Name"), Some("A & B"));
    assert_eq!(document.node(0).text, "head\nmore & raw\n");
    assert_eq!(document.node(0).children, vec![1, 3]);
    assert_eq!(document.node(0).parent, None);
    assert_eq!(document.node(2).parent, Some(1));
    assert_eq!(document.text(0, "Element/Value"), Some("1 > 0"));
    assert_eq!(document.find(0, ""), Some(0));
    assert_eq!(document.find(0, "Element/Missing"), None);
    assert_eq!(document.find(0, "/Element"), None);
    assert_eq!(document.descendants(0), vec![0, 1, 2, 3]);
    assert_eq!(document.descendants(1), vec![1, 2]);
    assert_eq!(document.named(0, "Element"), vec![1, 3]);
    assert_eq!(document.clone(), document);
}

#[test]
fn namespace_names_match_elementtree_expansion_and_invalid_documents_fail() {
    let document =
        Document::parse(br#"<a:Gauge xmlns:a="urn:gauge" a:id="7"><a:Value>1</a:Value></a:Gauge>"#)
            .unwrap();
    assert_eq!(document.node(0).tag, "{urn:gauge}Gauge");
    assert_eq!(document.node(0).attribute("{urn:gauge}id"), Some("7"));
    assert_eq!(document.child(0, "{urn:gauge}Value"), Some(1));
    assert_eq!(Document::parse(&[0xff]), Err(Error::InvalidUtf8));
    for input in [
        b"".as_slice(),
        b"<a><b></a>",
        br#"<!DOCTYPE e [<!ENTITY x "expanded">]><e>&x;</e>"#,
        b"<a>&unregistered;</a>",
        b"<a/><b/>",
    ] {
        assert!(
            matches!(Document::parse(input), Err(Error::Xml(_))),
            "{input:?}"
        );
    }
}

#[test]
fn tooltip_expressions_preserve_nested_quotes_and_unicode_source_offsets() {
    let text = "é: %((L:Value,number) '(' scmp)%{if}yes%{else}no%{end}";
    let parsed = template::parse(text).unwrap();
    assert_eq!(
        parsed.iter().map(|t| t.kind).collect::<Vec<_>>(),
        vec![
            Kind::Text,
            Kind::Expression,
            Kind::If,
            Kind::Text,
            Kind::Else,
            Kind::Text,
            Kind::End
        ]
    );
    assert_eq!(parsed[0].text, "é: ");
    assert_eq!(parsed[1].text, "(L:Value,number) '(' scmp");
    assert_eq!(parsed[1].offset, 5);
    assert_eq!(parsed[3].text, "yes");
}

#[test]
fn tooltip_escapes_labels_formats_and_unknown_sequences_keep_token_boundaries() {
    let tokens = template::parse("A%%B%nC% D%qE%{:12}%{other}%!03.2f!%!d!%").unwrap();
    assert_eq!(
        tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
        vec![
            Kind::Text,
            Kind::Text,
            Kind::Text,
            Kind::Text,
            Kind::Text,
            Kind::Label,
            Kind::Ignore,
            Kind::Format,
            Kind::Format
        ]
    );
    assert_eq!(
        tokens[..5]
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "%B", "\nC", " D", "qE"]
    );
    assert_eq!(tokens[5].number, 12);
    assert_eq!(tokens[6].text, "other");
    assert_eq!(tokens[7].text, "03.2f");
    assert_eq!(tokens[8].text, "d");
}

#[test]
fn malformed_templates_report_their_source_position() {
    for (text, message) in [
        ("é%(1", "unterminated expression"),
        ("%{if", "unterminated control tag"),
        ("%!2.1f", "unterminated numeric format"),
        ("%!0d!", "unsupported numeric format"),
        ("%{:2147483648}", "label exceeds integer range"),
    ] {
        let error = template::parse(text).unwrap_err();
        assert_eq!(error.message, message);
        assert!(error.offset > 0);
    }
    assert!(template::parse("no expressions").is_ok());
    assert!(template::parse("").unwrap().is_empty());
}
