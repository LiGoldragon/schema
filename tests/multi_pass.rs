//! Integration tests for the multi-pass NOTA-first schema reader.
//!
//! Per `reports/designer/334-multi-pass-nota-first-schema-reader.md` §7.

use std::path::{Path, PathBuf};

use schema::{AssembledSchema, LoadedSchema};

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/multi-pass")
        .join(rel)
}

fn run_multi_pass(path: &Path) -> AssembledSchema {
    schema::multi_pass::read_path(path).expect("multi-pass read failed")
}

fn run_canonical(path: &Path) -> AssembledSchema {
    LoadedSchema::read_path(path)
        .expect("canonical read failed")
        .assembled()
        .clone()
}

#[test]
fn multi_pass_reads_spirit_schema_with_full_equivalence() {
    let path = fixture("spirit.schema");
    let multi = run_multi_pass(&path);
    let canonical = run_canonical(&path);

    // Full byte-equivalence check via Debug equality. This is the
    // strongest claim the parallel-implementation prove-out can make.
    assert_eq!(
        format!("{multi:#?}"),
        format!("{canonical:#?}"),
        "AssembledSchema diverges between multi-pass and canonical readers"
    );
}

#[test]
fn multi_pass_reads_version_handover_schema_with_full_equivalence() {
    let path = fixture("version-handover.schema");
    let multi = run_multi_pass(&path);
    let canonical = run_canonical(&path);
    assert_eq!(
        format!("{multi:#?}"),
        format!("{canonical:#?}"),
        "AssembledSchema diverges for version-handover"
    );
}

#[test]
fn multi_pass_reads_orchestrate_schema_with_full_equivalence() {
    let path = fixture("orchestrate.schema");
    let multi = run_multi_pass(&path);
    let canonical = run_canonical(&path);
    assert_eq!(
        format!("{multi:#?}"),
        format!("{canonical:#?}"),
        "AssembledSchema diverges for orchestrate"
    );
}

#[test]
fn multi_pass_rejects_retired_path_import_form() {
    // (Path schemas/x.schema) — retired form — must not appear inside
    // a directive value. The canonical reader's parser also rejects
    // this. We check the multi-pass parser produces an error.
    let text = r#"
{
  Foo (Path schemas/x.schema)
}
[]
[]
[]
{}
[]
"#;
    let err = schema::multi_pass::read_str(text).unwrap_err();
    let message = format!("{err}");
    assert!(
        message.contains("unknown import directive `Path`"),
        "expected typed rejection of (Path ...), got: {message}"
    );
}

#[test]
fn multi_pass_rejects_scalar_header_root() {
    // (State Statement) without sub-list — retired form — must error.
    let text = r#"
{}
[(State Statement)]
[]
[]
{ State [Statement]
  Statement (String) }
[]
"#;
    let err = schema::multi_pass::read_str(text).unwrap_err();
    let message = format!("{err}");
    assert!(
        message.contains("requires a list of endpoints") || message.contains("v13 header root"),
        "expected typed rejection of scalar header form, got: {message}"
    );
}

#[test]
fn multi_pass_rejects_list_container_type_expression() {
    // [Option Topic] as a field type — retired form — must error.
    let text = r#"
{}
[]
[]
[]
{
  Topic (String)
  Container ([Option Topic])
}
[]
"#;
    let err = schema::multi_pass::read_str(text).unwrap_err();
    let message = format!("{err}");
    assert!(
        message.contains("list shape") || message.contains("(Vec T)"),
        "expected typed rejection of [T] container form, got: {message}"
    );
}
