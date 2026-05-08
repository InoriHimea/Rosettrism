use rosettrism::{decode_bytes, export_document, InputFormat, OutputFormat};

#[test]
fn decodes_sample_qrc_fixture_to_json_and_lrc() {
    let bytes = include_bytes!("fixtures/sample.qrc");
    let doc = decode_bytes(bytes, InputFormat::Auto).unwrap();

    assert_eq!(doc.meta.title.as_deref(), Some("Sample"));
    assert_eq!(doc.lines[0].start_ms, 1000);
    assert_eq!(doc.lines[0].words.len(), 2);

    let json = export_document(&doc, OutputFormat::Json).unwrap();
    assert!(String::from_utf8(json).unwrap().contains("\"words\""));

    let lrc = export_document(&doc, OutputFormat::Lrc).unwrap();
    assert!(String::from_utf8(lrc).unwrap().contains("[00:01.00]"));
}
