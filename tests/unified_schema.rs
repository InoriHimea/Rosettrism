use rosettrism::{UnifiedLyric, UnifiedLyricMode};
use serde_json::Value;

const SCHEMA: &str = include_str!("../schema/unified-lyric.schema.json");
const FIXTURES: &[(&str, &str)] = &[
    (
        "QQ/QRC tracks",
        include_str!("fixtures/unified/qq_qrc_tracks.json"),
    ),
    (
        "Kugou/KRC tracks",
        include_str!("fixtures/unified/kugou_krc_tracks.json"),
    ),
    (
        "Netease/YRC inline",
        include_str!("fixtures/unified/netease_yrc_inline.json"),
    ),
    (
        "LRCLIB/LRC inline",
        include_str!("fixtures/unified/lrclib_lrc_inline.json"),
    ),
];

#[test]
fn unified_schema_is_valid_json_and_declares_version() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    assert_eq!(schema["properties"]["schema_version"]["const"], "1.0");
    assert!(schema["$defs"]["LyricTrack"].is_object());
    assert!(schema["$defs"]["LyricDocument"].is_object());
    assert!(schema["$defs"]["LyricLine"].is_object());
    assert!(schema["$defs"]["Annotation"].is_object());
}

/// `schema_version` 三方一致性：model 常量 = schema 文件 const = 所有 fixture 值。
/// model 常量单独漂移（如 bump 未同步 schema 文件）时在此失败。
#[test]
fn schema_version_constant_matches_schema_file_and_fixtures() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let schema_const = schema["properties"]["schema_version"]["const"]
        .as_str()
        .expect("schema declares a const schema_version");
    assert_eq!(
        rosettrism::UNIFIED_LYRIC_SCHEMA_VERSION,
        schema_const,
        "model UNIFIED_LYRIC_SCHEMA_VERSION drifted from schema file const; \
         update both together and record the change in SCHEMA_CHANGELOG.md"
    );

    for (name, fixture) in FIXTURES {
        let value: Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            value["schema_version"],
            rosettrism::UNIFIED_LYRIC_SCHEMA_VERSION,
            "{name} fixture schema_version drifted from model constant"
        );
    }
}

#[test]
fn unified_fixtures_validate_against_schema_and_model() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();

    for (name, fixture) in FIXTURES {
        let value: Value = serde_json::from_str(fixture).unwrap_or_else(|err| {
            panic!("{name} fixture should be valid JSON: {err}");
        });
        validate_schema(&schema, &schema, &value, "$")
            .unwrap_or_else(|err| panic!("{name} fixture should validate: {err}"));

        let lyric: UnifiedLyric = serde_json::from_value(value).unwrap_or_else(|err| {
            panic!("{name} fixture should deserialize into UnifiedLyric: {err}");
        });
        assert_eq!(lyric.schema_version, "1.0");
        match lyric.mode {
            UnifiedLyricMode::Tracks => assert!(!lyric.tracks.is_empty()),
            UnifiedLyricMode::Inline => assert!(!lyric.inline_lines.is_empty()),
        }
    }
}

#[test]
fn missing_schema_version_deserializes_to_current_version_for_backcompat() {
    let fixture: Value = serde_json::from_str(FIXTURES[0].1).unwrap();
    let mut object = fixture.as_object().unwrap().clone();
    object.remove("schema_version");

    let lyric: UnifiedLyric = serde_json::from_value(Value::Object(object)).unwrap();
    assert_eq!(lyric.schema_version, "1.0");
}

fn validate_schema(root: &Value, schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let resolved = resolve_ref(root, reference)?;
        return validate_schema(root, resolved, value, path);
    }

    if let Some(expected) = schema.get("const") {
        if value != expected {
            return Err(format!("{path}: expected const {expected}, got {value}"));
        }
    }

    if let Some(options) = schema.get("enum").and_then(Value::as_array) {
        if !options.iter().any(|option| option == value) {
            return Err(format!("{path}: value {value} is not in enum"));
        }
    }

    if let Some(type_schema) = schema.get("type") {
        validate_type(type_schema, value, path)?;
    }

    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if let Some(number) = value.as_f64() {
            if number < minimum {
                return Err(format!("{path}: {number} is below minimum {minimum}"));
            }
        }
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        let object = value
            .as_object()
            .ok_or_else(|| format!("{path}: required applies to non-object"))?;
        for field in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(field) {
                return Err(format!("{path}: missing required field `{field}`"));
            }
        }
    }

    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        for (field, field_schema) in properties {
            if let Some(field_value) = object.get(field) {
                validate_schema(root, field_schema, field_value, &format!("{path}.{field}"))?;
            }
        }
    }

    if let (Some(items), Some(array)) = (schema.get("items"), value.as_array()) {
        for (index, item) in array.iter().enumerate() {
            validate_schema(root, items, item, &format!("{path}[{index}]"))?;
        }
    }

    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for sub_schema in all_of {
            if let Some(if_schema) = sub_schema.get("if") {
                if validate_schema(root, if_schema, value, path).is_ok() {
                    if let Some(then_schema) = sub_schema.get("then") {
                        validate_schema(root, then_schema, value, path)?;
                    }
                }
            } else {
                validate_schema(root, sub_schema, value, path)?;
            }
        }
    }

    Ok(())
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("unsupported non-local ref `{reference}`"))?;
    root.pointer(pointer)
        .ok_or_else(|| format!("unresolved schema ref `{reference}`"))
}

fn validate_type(type_schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(types) = type_schema.as_array() {
        if types
            .iter()
            .filter_map(Value::as_str)
            .any(|type_name| value_matches_type(value, type_name))
        {
            return Ok(());
        }
        return Err(format!(
            "{path}: value {value} did not match any allowed type"
        ));
    }

    if let Some(type_name) = type_schema.as_str() {
        if value_matches_type(value, type_name) {
            return Ok(());
        }
        return Err(format!(
            "{path}: value {value} did not match type `{type_name}`"
        ));
    }

    Ok(())
}

fn value_matches_type(value: &Value, type_name: &str) -> bool {
    match type_name {
        "array" => value.is_array(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "null" => value.is_null(),
        "number" => value.as_f64().is_some(),
        "object" => value.is_object(),
        "string" => value.is_string(),
        _ => false,
    }
}

const GOLDEN_DIR: &str = "tests/fixtures/golden";
const GOLDEN_CASES: &[(&str, &str)] = &[
    ("qq_qrc_tracks", "QQ/QRC tracks"),
    ("kugou_krc_tracks", "Kugou/KRC tracks"),
    ("netease_yrc_inline", "Netease/YRC inline"),
    ("lrclib_lrc_inline", "LRCLIB/LRC inline"),
];

/// Golden snapshot of the `UnifiedLyric` serialization contract.
///
/// 每个 fixture 反序列化为 `UnifiedLyric` 后重新序列化，输出与
/// `tests/fixtures/golden/<case>.snap` 对比。该快照固化了 model 的
/// 字段顺序、`skip_serializing_if` 空集合省略行为和 `schema_version`
/// 常量。任何破坏性序列化变更都会在 CI 上失败，迫使维护者在
/// `SCHEMA_CHANGELOG.md` 中登记并按主/次版本规则处理。
#[test]
fn golden_serialization_snapshots_match_model_round_trip() {
    let update = std::env::var("UPDATE_GOLDEN").as_deref() == Ok("1");
    std::fs::create_dir_all(GOLDEN_DIR).unwrap();

    for (case, fixture_name) in GOLDEN_CASES {
        let fixture_src = FIXTURES
            .iter()
            .find(|(name, _)| *name == *fixture_name)
            .map(|(_, src)| *src)
            .unwrap_or_else(|| {
                panic!("golden case {case} references missing fixture {fixture_name}")
            });
        let input: Value = serde_json::from_str(fixture_src).unwrap();
        let lyric: UnifiedLyric = serde_json::from_value(input).unwrap();
        let actual = serde_json::to_value(&lyric).unwrap();
        let actual_text = serde_json::to_string_pretty(&actual).unwrap();

        let snap_path = format!("{GOLDEN_DIR}/{case}.snap");
        if update {
            std::fs::write(&snap_path, &actual_text).unwrap();
            continue;
        }

        let expected_text = std::fs::read_to_string(&snap_path)
            .unwrap_or_else(|err| panic!(
                "golden snapshot {snap_path} missing: {err}\n\
                 Run `UPDATE_GOLDEN=1 cargo test --test unified_schema golden_serialization` to create it."
            ));
        assert_eq!(
            actual_text, expected_text,
            "serialization golden mismatch for {case}.\n\
             If this is intentional, update the snapshot:\n  \
             UPDATE_GOLDEN=1 cargo test --test unified_schema golden_serialization\n\
             and document the breaking change in SCHEMA_CHANGELOG.md."
        );
    }
    assert!(
        !update,
        "golden snapshots were regenerated; re-run without UPDATE_GOLDEN=1 to verify."
    );
}
