//! End-to-end demo of the schema-schema + NOTA library surface.
//!
//! Per Deliverable C — shows the path:
//!   schema text → NOTA library blocks → schema-schema interpretation
//!   → AssembledSchema. Prints the assembled output for review.
//!
//! The demo uses the `coordinate.schema` from the /354 prototype's
//! schemas directory. The .schema text contains NO explicit root
//! declaration — the `.schema` extension IS the declaration per
//! record 805.
//!
//! Exit 0 if the full chain succeeds, non-zero on diagnostic.

use schema_derived_nota_prototype::{
    AssembledNode, BlockParser, Classification, MacroContext, SchemaSchema,
};
use std::sync::Arc;

const COORDINATE_SCHEMA: &str = include_str!("../../schemas/coordinate.schema");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== schema-schema + NOTA library demo ===\n");

    // ── Step 1: Parse via the NOTA library surface ──────────────
    println!("[1] NOTA library — parse_blocks on coordinate.schema");
    let parser = BlockParser::new(COORDINATE_SCHEMA);
    let blocks = parser.parse_blocks()?;
    println!("    {} top-level blocks parsed", blocks.len());
    for (index, block) in blocks.iter().enumerate() {
        let classification = block.classification();
        let kind_label = match classification {
            Some(Classification::Block(kind)) => format!("Block({:?})", kind),
            Some(other) => format!("{:?}", other),
            None => "Unclassified".to_string(),
        };
        println!(
            "      block[{index}]: span={}..{}  kind={kind_label}  root_objects={}",
            block.span.start.byte_offset,
            block.span.end.byte_offset,
            block.holds_root_objects()
        );
    }

    // ── Step 2: Default schema-schema loads implicitly ──────────
    println!("\n[2] Default schema-schema — SchemaSchema::default()");
    let schema_schema = Arc::new(SchemaSchema::default());
    println!(
        "    {} built-in macros registered",
        schema_schema.builtin_macros().len()
    );
    for macro_ref in schema_schema.builtin_macros() {
        println!("      - {}", macro_ref.name());
    }

    // ── Step 3: Lower via positional macro dispatch ─────────────
    println!("\n[3] Lower via macro dispatch (positional)");
    let ctx = MacroContext::root(Arc::clone(&schema_schema));
    let lowered = schema_schema.lower_via_macros(COORDINATE_SCHEMA, &ctx)?;
    println!("    {} AssembledNode(s) emitted", lowered.len());
    for (index, node) in lowered.iter().enumerate() {
        match node {
            AssembledNode::ImportsTable { entries } => {
                println!("      [{index}] ImportsTable — {} entries", entries.len());
                for entry in entries {
                    println!("            {} -> ({})", entry.local_name, entry.source);
                }
            }
            AssembledNode::InputOutputStruct {
                input_operations, ..
            } => {
                println!(
                    "      [{index}] InputOutputStruct — {} operations",
                    input_operations.len()
                );
                for operation in input_operations {
                    println!(
                        "            ({}) payload-types: [{}]",
                        operation.tag,
                        operation.payload_types.join(", ")
                    );
                }
            }
            AssembledNode::Namespace { entries } => {
                println!("      [{index}] Namespace — {} declarations", entries.len());
                for entry in entries {
                    println!("            {} = {}", entry.name, entry.body_source);
                }
            }
            AssembledNode::Domain { tag, payload } => {
                println!("      [{index}] Domain({tag}): {payload}");
            }
        }
    }

    // ── Step 4: Full AssembledSchema via parse_schema_file ──────
    println!("\n[4] parse_schema_file → AssembledSchema (positional root struct)");
    let assembled = schema_schema.parse_schema_file(COORDINATE_SCHEMA)?;
    println!(
        "    input_operations: {} | output_operations: {} | namespace entries: {}",
        assembled.input_operations.len(),
        assembled.output_operations.len(),
        assembled.namespace.len()
    );
    print!("    input ops:");
    for operation in &assembled.input_operations {
        print!(" {}", operation.name);
    }
    println!();
    print!("    output ops:");
    for operation in &assembled.output_operations {
        print!(" {}", operation.name);
    }
    println!();

    // ── Step 5: Demonstrate the root-struct-implied claim ───────
    println!("\n[5] Root struct IMPLIED by .schema extension (record 805)");
    println!("    coordinate.schema's source contains NO (Schema ...) wrapping —");
    let has_schema_wrap = COORDINATE_SCHEMA.contains("(Schema ");
    let has_root_wrap = COORDINATE_SCHEMA.contains("(Root ");
    println!("      (Schema ...) present? {has_schema_wrap}   (Root ...) present? {has_root_wrap}");
    println!("    The five top-level blocks ARE the positional fields of the");
    println!("    implied root struct.");

    println!("\n=== demo complete ===");
    Ok(())
}
